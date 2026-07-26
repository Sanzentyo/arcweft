use super::*;
use crate::custom::ArcweftCustomRequest;
use crate::requests::SignatureRequestRuntime;
use arcweft_adapter_metadata::{
    AdapterFunctionExport, AdapterMetadata, AdapterParameter, AdapterTarget, AdapterTypeExport,
    AdapterTypeField, AdapterTypeShape, FunctionPurity,
};
use arcweft_manifest_model::{
    AdapterTypeName, FieldName, FunctionName, ManifestVisibility, RawDigest, TypeReference,
};
use lsp_server::{Connection, Message};
use lsp_types::{
    ClientCapabilities, CodeActionContext, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionResponse, InlayHint, InlayHintLabel, PartialResultParams, Position, Range,
    ReferenceContext, SignatureHelp, SignatureHelpParams, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri,
    VersionedTextDocumentIdentifier, WorkDoneProgressParams, WorkspaceClientCapabilities,
    WorkspaceEditClientCapabilities,
};
use std::{
    fs::{create_dir_all, write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

const ADAPTER_METADATA: &str =
    include_str!("../../../arcweft-adapter-metadata/tests/fixtures/truck-rust.adapter.json");

#[test]
fn capabilities_advertise_full_sync_and_p0_features() {
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let capabilities = session.initialize(&InitializeParams {
        capabilities: ClientCapabilities::default(),
        ..InitializeParams::default()
    });

    assert_eq!(
        capabilities.text_document_sync,
        Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL))
    );
    assert!(capabilities.hover_provider.is_some());
    assert!(capabilities.definition_provider.is_some());
    assert!(capabilities.references_provider.is_some());
    assert!(capabilities.completion_provider.is_some());
    assert!(capabilities.code_action_provider.is_some());
    assert!(capabilities.inlay_hint_provider.is_some());
}

#[test]
fn entry_definition_protocol_dispatch_honors_utf8_utf16_and_utf32_positions() {
    let project = TestProject::new("entry-definition-position-encodings");
    let source = r#"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    Ok(())
}

fn selected_entry() -> Unit {
    let selected = ("😀", @entry.agent.main)
    ()
}

entry agent @entry.agent.main {
    controller = smoke
}
"#;
    project.write(
        "arcw.toml",
        r#"schema = 1

[package]
id = "org.arcweft.tests.entry-definition-position-encodings"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
"#,
    );
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let reference = source.find("@entry.agent.main").expect("entry reference") + 1;

    for (kind, encoding) in [
        (
            lsp_types::PositionEncodingKind::UTF8,
            crate::positions::PositionEncoding::Utf8,
        ),
        (
            lsp_types::PositionEncodingKind::UTF16,
            crate::positions::PositionEncoding::Utf16,
        ),
        (
            lsp_types::PositionEncodingKind::UTF32,
            crate::positions::PositionEncoding::Utf32,
        ),
    ] {
        let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("agent"));
        let capabilities = session.initialize(&InitializeParams {
            capabilities: ClientCapabilities {
                general: Some(lsp_types::GeneralClientCapabilities {
                    position_encodings: Some(vec![kind.clone()]),
                    ..lsp_types::GeneralClientCapabilities::default()
                }),
                ..ClientCapabilities::default()
            },
            ..InitializeParams::default()
        });
        assert_eq!(capabilities.position_encoding, Some(kind));
        open_text(&mut session, uri.clone(), source);
        let line_index = crate::positions::LineIndex::new(source.to_owned(), encoding);
        let response = session.handle_request(Request {
            id: RequestId::from(41),
            method: GotoDefinition::METHOD.to_owned(),
            params: serde_json::json!(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: line_index.position_from_byte_offset(reference),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            }),
        });
        assert!(response.error.is_none(), "{:?}", response.error);
        let definition = serde_json::from_value::<GotoDefinitionResponse>(
            response.result.expect("definition response"),
        )
        .expect("definition response decodes");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("entry definition is a scalar location");
        };
        assert_eq!(location.uri, uri);
    }
}

#[test]
fn workspace_edit_normalization_is_deterministic_and_deduplicated() {
    let current = "file:///b.arcw".parse::<Uri>().expect("current URI");
    let other = "file:///a.arcw".parse::<Uri>().expect("other URI");
    let mut documents = crate::documents::DocumentStore::default();
    documents.open(
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                current.clone(),
                "arcweft".to_owned(),
                7,
                String::new(),
            ),
        },
        PositionEncoding::Utf16,
    );
    documents.open(
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                other.clone(),
                "arcweft".to_owned(),
                9,
                String::new(),
            ),
        },
        PositionEncoding::Utf16,
    );
    let later = lsp_types::TextEdit::new(
        Range::new(Position::new(3, 4), Position::new(3, 8)),
        "later".to_owned(),
    );
    let earlier = lsp_types::TextEdit::new(
        Range::new(Position::new(1, 2), Position::new(1, 6)),
        "earlier".to_owned(),
    );
    let edit = lsp_types::WorkspaceEdit {
        changes: Some(std::collections::HashMap::from([
            (current.clone(), vec![later.clone(), earlier.clone(), later]),
            (other.clone(), vec![earlier]),
        ])),
        document_changes: None,
        change_annotations: None,
    };
    let normalized = WorkspaceEditPolicy {
        document_changes: true,
    }
    .normalize(edit, &documents);
    let Some(lsp_types::DocumentChanges::Edits(edits)) = normalized.document_changes else {
        panic!("normalized document edits");
    };
    assert_eq!(
        edits
            .iter()
            .map(|edit| edit.text_document.uri.clone())
            .collect::<Vec<_>>(),
        [other, current]
    );
    assert_eq!(edits[1].edits.len(), 2);
    assert_eq!(edits[0].text_document.version, Some(9));
    assert_eq!(edits[1].text_document.version, Some(7));
    let lsp_types::OneOf::Left(first) = &edits[1].edits[0] else {
        panic!("plain text edit");
    };
    assert_eq!(first.range.start, Position::new(1, 2));
}

#[test]
fn full_sync_notifications_publish_diagnostics() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());

    let open = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "arcweft".to_owned(),
                version: 1,
                text: "flow @flow.opening opening {\n".to_owned(),
            },
        },
    );
    let notifications = session.handle_notification(open).expect("open");

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].method, PublishDiagnostics::METHOD);

    let change = Notification::new(
        DidChangeTextDocument::METHOD.to_owned(),
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version: 2 },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flow @flow.opening opening {}\n".to_owned(),
            }],
        },
    );
    let notifications = session.handle_notification(change).expect("change");

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].method, PublishDiagnostics::METHOD);
}

#[test]
fn semantic_analysis_cache_is_exact_reused_and_bounded_per_open_uri() {
    let project = TestProject::new("lsp-analysis-cache");
    project.write(
        "arcw.toml",
        &canonical_project_manifest("lsp-analysis-cache", ""),
    );
    let first = "pub character @character.alice Alice as alice {}\nflow main {\n  alice: one\n}\n";
    project.write("src/main.arcw", first);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), first);

    assert_eq!(session.analyses_by_uri.len(), 1);
    let first_analysis = Arc::clone(
        &session
            .analyses_by_uri
            .get(&crate::uri_key::LspUriKey::from_uri(&uri))
            .expect("open analysis")
            .analysis,
    );
    assert_code_actions_reuse_analysis(&session, &uri, &first_analysis);

    let second = first.replace("alice: one", "alice: two");
    session
        .handle_notification(Notification::new(
            DidChangeTextDocument::METHOD.to_owned(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: second,
                }],
            },
        ))
        .expect("full text change");
    assert_eq!(session.analyses_by_uri.len(), 1);
    let changed = Arc::clone(
        &session
            .analyses_by_uri
            .get(&crate::uri_key::LspUriKey::from_uri(&uri))
            .expect("changed analysis")
            .analysis,
    );
    assert!(!Arc::ptr_eq(&first_analysis, &changed));

    let saved_analysis = assert_notification_rebuilds_analysis(
        &mut session,
        &uri,
        Notification::new(
            DidSaveTextDocument::METHOD.to_owned(),
            DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                text: None,
            },
        ),
        &changed,
        "save document",
    );
    let configuration_analysis = assert_notification_rebuilds_analysis(
        &mut session,
        &uri,
        Notification::new(
            DidChangeConfiguration::METHOD.to_owned(),
            DidChangeConfigurationParams {
                settings: serde_json::Value::Null,
            },
        ),
        &saved_analysis,
        "configuration change",
    );
    assert_notification_rebuilds_analysis(
        &mut session,
        &uri,
        Notification::new(
            DidChangeWatchedFiles::METHOD.to_owned(),
            DidChangeWatchedFilesParams {
                changes: Vec::new(),
            },
        ),
        &configuration_analysis,
        "project context refresh",
    );

    session
        .handle_notification(Notification::new(
            DidCloseTextDocument::METHOD.to_owned(),
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri },
            },
        ))
        .expect("close document");
    assert!(session.analyses_by_uri.is_empty());
}

fn assert_notification_rebuilds_analysis(
    session: &mut ArcweftLspSession,
    uri: &Uri,
    notification: Notification,
    previous: &Arc<DocumentAnalysis>,
    context: &str,
) -> Arc<DocumentAnalysis> {
    let previous_generation = session
        .profile_for_uri(uri)
        .accepted_environment()
        .map(|environment| environment.generation().get());
    session.handle_notification(notification).expect(context);
    let current_generation = session
        .profile_for_uri(uri)
        .accepted_environment()
        .map(|environment| environment.generation().get());
    if let (Some(previous), Some(current)) = (previous_generation, current_generation) {
        assert!(current > previous);
    }
    assert_eq!(session.analyses_by_uri.len(), 1);
    let current = Arc::clone(
        &session
            .analyses_by_uri
            .get(&crate::uri_key::LspUriKey::from_uri(uri))
            .expect("rebuilt analysis")
            .analysis,
    );
    assert!(!Arc::ptr_eq(previous, &current));
    current
}

fn assert_code_actions_reuse_analysis(
    session: &ArcweftLspSession,
    uri: &Uri,
    expected: &Arc<DocumentAnalysis>,
) {
    let _ = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(4, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("cached code actions");
    assert!(Arc::ptr_eq(
        expected,
        &session
            .analyses_by_uri
            .get(&crate::uri_key::LspUriKey::from_uri(uri))
            .expect("reused analysis")
            .analysis
    ));
}

#[test]
fn code_actions_expand_effect_upper_bound() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(
        &mut session,
        uri.clone(),
        r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}
flow @flow.opening opening
effects { }
{
    let body = fs.read_text(path = "story.arcw")
}
"#,
    );

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(20, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = code_action_by_title(&actions, "Expand effect upper bound")
        .expect("upper-bound quickfix exists");
    assert!(workspace_edit_replacements(action).contains(&"effects { fs.read }".to_owned()));
}

#[test]
fn code_actions_do_not_remove_unused_effect_upper_bound() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(
        &mut session,
        uri.clone(),
        r#"
flow @flow.opening opening
effects { fs.read, debug.record }
{
    return "ok"
}
"#,
    );

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(20, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    assert!(
        code_action_by_title(&actions, "Remove unused effect declaration").is_none(),
        "unused upper-bound members are not diagnostics"
    );
}

#[test]
fn code_actions_include_canonical_rich_text_rewrite() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(
        &mut session,
        uri.clone(),
        "flow @flow.opening opening {\n    alice: [.keyword]word[/][.sparkle amp=2px]hi[/]\n}\n",
    );

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(10, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Canonicalize inferred rich-text tags" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("canonical rich-text action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert!(
        edits[0]
            .new_text
            .contains("[mark .keyword]word[effect .sparkle amp=2px]hi[/effect]")
    );
    assert!(
        edits[0]
            .new_text
            .contains("[effect .sparkle amp=2px]hi[/effect]")
    );
    assert!(!edits[0].new_text.contains("[/]"));
}

#[test]
fn code_actions_canonical_rich_text_preserve_nested_proxy_params() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(
        &mut session,
        uri.clone(),
        "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\n#[text_proxy(kind=\"hover\", default_hit=false)]\npub struct HoverHit {\n    layer: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=inventory][.HoverHit tone=alert]multi[/][/]\n}\n",
    );

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(14, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Canonicalize inferred rich-text tags" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("canonical rich-text action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert!(edits[0].new_text.contains(
            "[object .hotspot type=KeywordHit channel=inventory][object .HoverHit type=HoverHit tone=alert]multi[/object][/object]"
        ));
    assert!(!edits[0].new_text.contains("[effect .HoverHit"));
    assert!(!edits[0].new_text.contains("[/]"));
}

#[test]
fn code_actions_canonicalize_sugar_respects_decl_identity_attributes() {
    let project = TestProject::new("lsp-checked-canonicalize-identities");
    project.write(
        "arcw.toml",
        &canonical_project_manifest("lsp-canonicalize-identities", ""),
    );
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = "#[generated]\nflow @flow.generated generated {\n}\n#[allow(style::redundant_decl_identity)]\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\nflow @flow.opening opening {\n}\nflow @flow.opening start {\n}\n";
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    open_text(&mut session, uri.clone(), source);

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(10, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Canonicalize Arcweft sugar" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("expand sugar action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    let rewritten = &edits[0].new_text;
    assert!(rewritten.contains("flow @flow.generated generated {"));
    assert!(rewritten.contains("source @source.http_requests http_requests"));
    assert!(rewritten.contains("flow opening {"));
    assert!(rewritten.contains("flow @flow.opening start {"));
}

#[test]
fn code_actions_shared_helper_corpus_matches_tooling_output() {
    let project = TestProject::new("lsp-checked-canonicalize-shared-helper");
    project.write(
        "arcw.toml",
        &canonical_project_manifest("lsp-canonicalize-shared-helper", ""),
    );
    let source = include_str!(
        "../../../arcweft-tooling/tests/fixtures/canonicalization/aw-ah-003-helper.arcw"
    );
    let expected = include_str!(
        "../../../arcweft-tooling/tests/fixtures/canonicalization/aw-ah-003-helper.expected.arcw"
    );
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(8, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");
    let action = code_action_by_title(&actions, "Canonicalize Arcweft sugar")
        .expect("semantic canonicalization action");
    let replacements = workspace_edit_replacements(action);

    assert_eq!(replacements, [expected.to_owned()]);
}

#[test]
fn code_actions_canonicalize_sugar_respects_source_allow_decl_identity_attribute() {
    let project = TestProject::new("lsp-checked-canonicalize-source-allow");
    project.write(
        "arcw.toml",
        &canonical_project_manifest("lsp-canonicalize-source-allow", ""),
    );
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = "#![allow(style::redundant_decl_identity)]\npub character @character.alice Alice as alice {}\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    open_text(&mut session, uri.clone(), source);

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(4, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Canonicalize Arcweft sugar" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("expand sugar action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    let rewritten = &edits[0].new_text;
    assert!(rewritten.contains("flow @flow.generated generated {"));
    assert!(rewritten.contains("alice.say()[hi[p]]"));
}

#[test]
fn code_actions_extract_active_style_contributor_to_line_options() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#202122")
            }
        }
    }
}

flow opening {
    alice: |[夢](ゆめ)[p]
}
"##;
    let (_project, session, uri) = accepted_dialogue_session("action-line-style", source);
    let document = session.documents.get(&uri).expect("open document");
    let offset = source.find("夢").expect("dialogue content");
    let position = document.line_index().position_from_byte_offset(offset);

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(position, position),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Extract `rich_text.text.color` override to line options" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("text color extraction action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "(rich_text.text.color=rgb(\"#202122\"))");
}

#[test]
fn unaccepted_profile_does_not_offer_speaker_preset_cascade_action() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r"
pub character alice {
    dialogue_style {
        rich_text {
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    let alice_side = alice(voice=auto)
    alice_side: |[夢](ゆめ)[p]
}
";
    open_text(&mut session, uri.clone(), source);
    let document = session.documents.get(&uri).expect("open document");
    let offset = source.find("夢").expect("dialogue content");
    let position = document.line_index().position_from_byte_offset(offset);

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(position, position),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = actions.iter().find_map(|action| match action {
        CodeActionOrCommand::CodeAction(action)
            if action.title == "Extract `rich_text.ruby.size` override to speaker preset" =>
        {
            Some(action)
        }
        CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
    });
    assert!(action.is_none());
}

#[test]
fn execute_command_can_return_workspace_edit_from_tooling_edit_argument() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_fixture(&mut session, uri.clone());
    let edit = arcweft_tooling::model::TextEdit {
        start: 0,
        end: 0,
        replacement: "// generated\n".to_owned(),
    };

    let result = session.execute_command(&ExecuteCommandParams {
        command: ArcweftCommand::ExpandSugar.as_str().to_owned(),
        arguments: vec![serde_json::json!({
            "uri": uri.to_string(),
            "edit": edit
        })],
        work_done_progress_params: WorkDoneProgressParams::default(),
    });

    let workspace_edit: lsp_types::WorkspaceEdit =
        serde_json::from_value(result).expect("workspace edit result");
    assert!(
        workspace_edit
            .changes
            .expect("changes fallback")
            .values()
            .any(|edits| !edits.is_empty())
    );
}

#[test]
fn execute_command_rejects_old_positional_tooling_edit_arguments() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_fixture(&mut session, uri.clone());
    let edit = arcweft_tooling::model::TextEdit {
        start: 0,
        end: 0,
        replacement: "// generated\n".to_owned(),
    };

    let result = session.execute_command(&ExecuteCommandParams {
        command: ArcweftCommand::ExpandSugar.as_str().to_owned(),
        arguments: vec![serde_json::json!(uri.to_string()), serde_json::json!(edit)],
        work_done_progress_params: WorkDoneProgressParams::default(),
    });

    assert_eq!(result, serde_json::Value::Null);
}

#[test]
fn execute_command_uses_document_changes_when_client_supports_them() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    session.initialize(&InitializeParams {
        capabilities: ClientCapabilities {
            workspace: Some(WorkspaceClientCapabilities {
                workspace_edit: Some(WorkspaceEditClientCapabilities {
                    document_changes: Some(true),
                    ..WorkspaceEditClientCapabilities::default()
                }),
                ..WorkspaceClientCapabilities::default()
            }),
            ..ClientCapabilities::default()
        },
        ..InitializeParams::default()
    });
    open_fixture(&mut session, uri.clone());
    let edit = arcweft_tooling::model::TextEdit {
        start: 0,
        end: 0,
        replacement: "// generated\n".to_owned(),
    };

    let result = session.execute_command(&ExecuteCommandParams {
        command: ArcweftCommand::ExpandSugar.as_str().to_owned(),
        arguments: vec![serde_json::json!({
            "uri": uri.to_string(),
            "edit": edit
        })],
        work_done_progress_params: WorkDoneProgressParams::default(),
    });

    let workspace_edit: lsp_types::WorkspaceEdit =
        serde_json::from_value(result).expect("workspace edit result");
    assert!(workspace_edit.changes.is_none());
    assert!(matches!(
        workspace_edit.document_changes,
        Some(lsp_types::DocumentChanges::Edits(_))
    ));
}

#[test]
fn repl_command_request_without_executor_returns_typed_host_unavailable() {
    let mut session = ArcweftLspSession::new(&LspConfig::default());

    let response = session.handle_request(Request {
        id: RequestId::from(12),
        method: ArcweftCustomRequest::ReplCommand.as_str().to_owned(),
        params: serde_json::json!({
            "input": ":tasks",
            "command_id": 77,
        }),
    });

    assert!(response.error.is_none());
    let result = response.result.expect("LSP REPL command response");
    assert_eq!(result["is_error"], serde_json::json!(true));
    assert_eq!(result["result"]["command_id"], serde_json::json!(77));
    assert_eq!(result["result"]["status"], serde_json::json!("error"));
    assert_eq!(
        result["diagnostics"][0]["code"],
        serde_json::json!("host_unavailable")
    );
    assert_eq!(
        result["result"]["evidence"]["kind"],
        serde_json::json!("empty")
    );
}

#[test]
fn did_open_refreshes_project_profile_for_completion() {
    let project = TestProject::new("lsp-session-profile");
    let metadata = adapter_metadata_with_function("echo", "String", "String");
    project.write(
        "arcw.toml",
        &external_module_project_manifest(
            "lsp-session-profile",
            "custom-echo",
            "custom",
            "generated/custom-echo.adapter.json",
            &metadata,
        ),
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("generated/custom-echo.adapter.json", &metadata);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), "flow @.main main {}\n");

    let completions = completion_labels(&mut session, uri);
    assert!(completions.iter().any(|item| item.label == "custom.echo"));
}

#[test]
fn completions_include_standard_enum_variant_shorthands() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), "flow @flow.opening opening {}\n");

    let completions = completion_labels(&mut session, uri);
    let json = completions
        .iter()
        .find(|item| item.label == ".Json")
        .expect(".Json completion");

    assert_eq!(json.kind, Some(lsp_types::CompletionItemKind::ENUM_MEMBER));
    assert_eq!(json.detail.as_deref(), Some("DataFormat.Json"));
    assert!(completions.iter().any(|item| item.label == ".MessagePack"));
}

#[test]
fn completions_and_hover_expose_standard_dialogue_view_nominal_contract() {
    let uri = "file:///dialogue-view.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r"
pub view DialoguePanel(dialogue: DialogueView) {
    Column {
        Text(dialogue.speaker)
        RichText(dialogue.content)
    }
}
";
    open_text(&mut session, uri.clone(), source);

    let completions = completion_labels(&mut session, uri.clone());
    for expected in [
        "DialogueView",
        "speaker",
        "content",
        "occurrence",
        "stage",
        "reveal",
        "primary_action",
    ] {
        assert!(
            completions.iter().any(|item| item.label == expected),
            "missing `{expected}` completion: {completions:?}"
        );
    }

    let hover = hover_text(&mut session, uri, source, "DialogueView");
    assert!(hover.contains("#[dialogue_view]"));
    assert!(hover.contains("primary_action: DialogueAction"));
}

#[test]
fn completion_and_hover_use_custom_dialogue_view_role_inventory() {
    let source = r"
#[dialogue_view]
pub struct StoryDialogue {
    speaker: String
    content: DialogueContent
    occurrence: DialogueOccurrenceId
    stage: DialogueStage
    reveal: DialogueReveal
    primary_action: DialogueAction
}
";
    let (_project, mut session, uri) = accepted_dialogue_session("custom-dialogue-view", source);

    let completions = completion_labels(&mut session, uri.clone());
    assert!(
        completions.iter().any(|item| item.label == "StoryDialogue"),
        "custom role model is absent: {completions:?}"
    );
    let hover = hover_text(&mut session, uri, source, "StoryDialogue");
    assert!(
        hover.contains("pub struct StoryDialogue"),
        "unexpected custom dialogue View hover: {hover}"
    );
    assert!(
        hover.contains("occurrence: DialogueOccurrenceId"),
        "unexpected custom dialogue View hover: {hover}"
    );
}

#[test]
fn completions_use_document_scoped_profiles() {
    let alpha = TestProject::new("lsp-session-alpha");
    let alpha_metadata = adapter_metadata_with_function("call", "String", "String");
    alpha.write(
        "arcw.toml",
        &external_module_project_manifest(
            "lsp-session-alpha",
            "alpha",
            "alpha",
            "generated/alpha.adapter.json",
            &alpha_metadata,
        ),
    );
    alpha.write("src/main.arcw", "flow @.main main {}\n");
    alpha.write("generated/alpha.adapter.json", &alpha_metadata);
    let beta = TestProject::new("lsp-session-beta");
    let beta_metadata = adapter_metadata_with_function("call", "String", "String");
    beta.write(
        "arcw.toml",
        &external_module_project_manifest(
            "lsp-session-beta",
            "beta",
            "beta",
            "generated/beta.adapter.json",
            &beta_metadata,
        ),
    );
    beta.write("src/main.arcw", "flow @.main main {}\n");
    beta.write("generated/beta.adapter.json", &beta_metadata);
    let alpha_uri = file_uri(&alpha.path("src/main.arcw"));
    let beta_uri = file_uri(&beta.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, alpha_uri.clone(), "flow @.main main {}\n");
    open_text(&mut session, beta_uri.clone(), "flow @.main main {}\n");

    let alpha_completions = completion_labels(&mut session, alpha_uri);
    let beta_completions = completion_labels(&mut session, beta_uri);

    assert!(
        alpha_completions
            .iter()
            .any(|item| item.label == "alpha.call")
    );
    assert!(
        !alpha_completions
            .iter()
            .any(|item| item.label == "beta.call")
    );
    assert!(
        beta_completions
            .iter()
            .any(|item| item.label == "beta.call")
    );
    assert!(
        !beta_completions
            .iter()
            .any(|item| item.label == "alpha.call")
    );
}

#[test]
fn watched_file_change_refreshes_profile_metadata() {
    let project = TestProject::new("lsp-session-watch-refresh");
    let before_metadata = adapter_metadata_with_function("before", "String", "String");
    project.write(
        "arcw.toml",
        &external_module_project_manifest(
            "lsp-session-watch-refresh",
            "custom",
            "custom",
            "generated/custom.adapter.json",
            &before_metadata,
        ),
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("generated/custom.adapter.json", &before_metadata);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), "flow @.main main {}\n");
    assert!(
        completion_labels(&mut session, uri.clone())
            .iter()
            .any(|item| item.label == "custom.before")
    );

    let after_metadata = adapter_metadata_with_function("after", "String", "String");
    project.write("generated/custom.adapter.json", &after_metadata);
    project.write(
        "arcw.toml",
        &external_module_project_manifest(
            "lsp-session-watch-refresh",
            "custom",
            "custom",
            "generated/custom.adapter.json",
            &after_metadata,
        ),
    );
    let refresh = Notification::new(
        DidChangeWatchedFiles::METHOD.to_owned(),
        DidChangeWatchedFilesParams {
            changes: Vec::new(),
        },
    );
    let notifications = session.handle_notification(refresh).expect("refresh");

    assert_eq!(notifications.len(), 1);
    let completions = completion_labels(&mut session, uri);
    assert!(completions.iter().any(|item| item.label == "custom.after"));
    assert!(!completions.iter().any(|item| item.label == "custom.before"));
}

#[test]
fn watched_file_change_refreshes_external_module_metadata() {
    let project = TestProject::new("lsp-session-rust-watch-refresh");
    let metadata = quest_adapter_metadata();
    project.write(
        "arcw.toml",
        &external_module_project_manifest(
            "lsp-session-rust-watch-refresh",
            "quest",
            "quest",
            "generated/quest.adapter.json",
            &metadata,
        ),
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("generated/quest.adapter.json", "{ not json");
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), "flow @.main main {}\n");
    assert!(
        !completion_labels(&mut session, uri.clone())
            .iter()
            .any(|item| item.label == "quest.quest_evaluate")
    );

    project.write("generated/quest.adapter.json", &metadata);
    let refresh = Notification::new(
        DidChangeWatchedFiles::METHOD.to_owned(),
        DidChangeWatchedFilesParams {
            changes: Vec::new(),
        },
    );
    let notifications = session.handle_notification(refresh).expect("refresh");

    assert_eq!(notifications.len(), 1);
    assert!(
        completion_labels(&mut session, uri)
            .iter()
            .any(|item| item.label == "quest.quest_evaluate")
    );
}

#[test]
fn session_reads_external_module_metadata_for_completion_and_hover() {
    let project = TestProject::new("lsp-session-rust-metadata");
    let metadata = quest_adapter_metadata();
    project.write(
        "arcw.toml",
        &external_module_project_manifest(
            "lsp-session-rust-metadata",
            "quest",
            "quest",
            "generated/quest.adapter.json",
            &metadata,
        ),
    );
    project.write("generated/quest.adapter.json", &metadata);
    let source = "entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), source);
    assert!(
        session
            .profile_for_uri(&uri)
            .accepted_environment()
            .is_some(),
        "external-module profile was not accepted: {:#?}",
        session.profile_for_uri(&uri).diagnostics()
    );

    let completions = completion_labels(&mut session, uri.clone());
    assert_eq!(
        completions
            .iter()
            .filter(|item| item.label == "quest.PlayerStats")
            .count(),
        1,
        "the accepted nominal catalog is the single completion owner: {:?}",
        completions
            .iter()
            .filter(|item| item.label == "quest.PlayerStats")
            .collect::<Vec<_>>()
    );
    let player_stats = completions
        .iter()
        .find(|item| item.label == "quest.PlayerStats")
        .expect("mounted PlayerStats completion");
    assert_eq!(
        player_stats.detail.as_deref(),
        Some("environment:adapter:sans-io::quest.PlayerStats")
    );
    let evaluate = completions
        .iter()
        .find(|item| item.label == "quest.quest_evaluate")
        .expect("mounted quest_evaluate completion");
    assert!(
        evaluate.detail.as_deref().is_some_and(|detail| {
            detail == "quest.quest_evaluate(stats: quest.PlayerStats) -> String"
        }),
        "unexpected callable completion: {evaluate:?}"
    );

    let hover = arcweft_verify_lsp::profile_hover(
        &session.profile_for_uri(&uri).context(),
        "quest.PlayerStats",
    )
    .expect("mounted type hover");
    assert!(matches!(
        hover.contents,
        lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(text))
            if text == "quest.PlayerStats: quest.PlayerStats"
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the end-to-end test verifies result projection plus hit, miss, stable-none, and error cache behavior"
)]
fn signature_help_uses_native_registered_adapter_candidate() {
    let project = TestProject::new("lsp-session-signature-adapter");
    project.write(
        "arcw.toml",
        r#"
schema = 1

[package]
id = "org.arcweft.tests.lsp.signature"
version = "0.1.0"

[profiles.dev]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
adapter = "inference-tensor"
"#,
    );
    let source = "fn evaluate_tensor(value: TensorF32) -> TensorF32 {\n    infer.add_f32(value, value)\n}\n\
entry server @entry.server.main { goto @flow.main }\n\
flow @flow.main main {}\n";
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), source);
    let session = Arc::new(RwLock::new(session));
    let (server, client) = Connection::memory();
    let runtime =
        SignatureRequestRuntime::new(&server, Arc::clone(&session)).expect("signature runtime");

    let response = native_signature_response(
        &session,
        &runtime,
        &client,
        3,
        uri.clone(),
        position_after(source, "infer.add_f32("),
    );
    let first_result = response.result.expect("signature help response");
    let signature: SignatureHelp =
        serde_json::from_value(first_result.clone()).expect("signature help response decodes");

    let first = signature.signatures.first().expect("signature item");
    assert_eq!(
        first.label,
        "infer.add_f32(lhs: TensorF32, rhs: TensorF32) -> TensorF32"
    );
    assert!(matches!(
        first.documentation.as_ref(),
        Some(lsp_types::Documentation::MarkupContent(content))
            if content.value.contains("Canonical owner: `InferApi.add_f32`.")
    ));
    assert_eq!(
        first.parameters.as_ref().expect("parameters"),
        &[
            lsp_types::ParameterInformation {
                label: lsp_types::ParameterLabel::LabelOffsets([14, 28]),
                documentation: None,
            },
            lsp_types::ParameterInformation {
                label: lsp_types::ParameterLabel::LabelOffsets([30, 44]),
                documentation: None,
            },
        ]
    );
    assert_eq!(signature.active_signature, Some(0));
    assert_eq!(signature.active_parameter, Some(0));

    let accepted = session
        .read()
        .expect("session read")
        .profile_for_uri(&uri)
        .accepted_environment()
        .expect("accepted environment");
    let first_cache = accepted.signature_cache_snapshot_for_test();
    assert_eq!(first_cache.entries, 1);
    assert_eq!(first_cache.misses, 1);
    assert_eq!(first_cache.insertions, 1);
    assert_eq!(first_cache.hits, 0);

    let cached = native_signature_response(
        &session,
        &runtime,
        &client,
        4,
        uri.clone(),
        position_after(source, "infer.add_f32("),
    );
    assert_eq!(cached.result, Some(first_result));
    let cached_snapshot = accepted.signature_cache_snapshot_for_test();
    assert_eq!(cached_snapshot.entries, 1);
    assert_eq!(cached_snapshot.misses, 1);
    assert_eq!(cached_snapshot.insertions, 1);
    assert_eq!(cached_snapshot.hits, 1);

    let outside = native_signature_response(
        &session,
        &runtime,
        &client,
        5,
        uri.clone(),
        position_of(source, "infer.add_f32"),
    );
    assert_eq!(outside.result, Some(serde_json::Value::Null));
    assert!(outside.error.is_none());
    let outside_cached = native_signature_response(
        &session,
        &runtime,
        &client,
        6,
        uri.clone(),
        position_of(source, "infer.add_f32"),
    );
    assert_eq!(outside_cached.result, Some(serde_json::Value::Null));
    assert!(outside_cached.error.is_none());
    let stable_none = accepted.signature_cache_snapshot_for_test();
    assert_eq!(stable_none.entries, 2);
    assert_eq!(stable_none.misses, 2);
    assert_eq!(stable_none.insertions, 2);
    assert_eq!(stable_none.hits, 2);

    let invalid = native_signature_response(
        &session,
        &runtime,
        &client,
        7,
        uri,
        Position::new(u32::MAX, 0),
    );
    let error = invalid.error.expect("invalid position error");
    assert_eq!(error.code, ErrorCode::InvalidParams as i32);
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "aw.signature.request.invalid_lsp_position"
        }))
    );
    assert_eq!(
        accepted.signature_cache_snapshot_for_test(),
        stable_none,
        "invalid positions never mutate the semantic cache"
    );

    runtime.shutdown();
}

#[test]
fn inlay_hint_request_reports_inferred_function_types() {
    let uri = "file:///function-inlays.arcw".parse::<Uri>().expect("uri");
    let source = r"
flow @flow.function_inlays function_inlays {
    let predicate = _ > 80i64
    let zero = || 1
    let explicit: i64 -> bool = _ > 80i64
}
";
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

    let labels = inlay_hint_labels(&mut session, uri);

    assert!(
        labels.iter().any(|label| label == ": i64 -> bool"),
        "expected inferred partial-placeholder function type inlay, got {labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == ": () -> i32"),
        "expected inferred closure function type inlay, got {labels:?}"
    );
    assert_eq!(
        labels
            .iter()
            .filter(|label| label.as_str() == ": i64 -> bool")
            .count(),
        1,
        "explicit let type ascription should not produce a duplicate inlay: {labels:?}"
    );
}

#[test]
fn inlay_hint_request_reports_unsuffixed_numeric_fallback_types() {
    let uri = "file:///numeric-inlays.arcw".parse::<Uri>().expect("uri");
    let source = r"
flow @flow.numeric_inlays numeric_inlays {
    let count = 42
    let ratio = 1_2.5_0
    let negative = -1
    let total = 1 + 2
    let values = [1, 2]
    let explicit: u64 = 42
}
";
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

    let labels = inlay_hint_labels(&mut session, uri);

    assert!(labels.iter().any(|label| label == ": i32"), "{labels:?}");
    assert!(labels.iter().any(|label| label == ": f64"), "{labels:?}");
    assert!(
        labels.iter().any(|label| label == ": Vec<i32>"),
        "numeric sequence fallback should expose its container type: {labels:?}"
    );
    assert!(
        labels
            .iter()
            .filter(|label| label.as_str() == ": i32")
            .count()
            >= 3,
        "literal, unary, and binary fallback sites should all receive hints: {labels:?}"
    );
    assert!(
        !labels.iter().any(|label| label == ": u64"),
        "explicit numeric type should not receive a duplicate inlay: {labels:?}"
    );
}

#[test]
fn expression_type_inlays_are_profile_gated_and_skip_trivial_sites() {
    let source = r#"
struct Choice {
    label: String,
    enabled: bool,
}

fn add(lhs: i64, rhs: i64) -> i64 { lhs + rhs }

flow @flow.expression_inlays expression_inlays {
    let base = 2i64
    let copy = base
    let choice = Choice { label: "Start", enabled: true }
    let label = choice.label
    let total = add(1i64, base + 3i64)
    let piped = base |> add(^, 4i64)
}
"#;

    let (_project, mut default_session, uri) =
        accepted_dialogue_session("expression-inlays", source);
    let default_labels = inlay_hint_labels(&mut default_session, uri.clone());
    assert!(
        !default_labels.iter().any(|label| label == ": i64"),
        "expression type inlays should be opt-in, got {default_labels:?}"
    );

    let mut enabled_session = ArcweftLspSession::new(
        &LspConfig::default()
            .with_profile_id("dev")
            .with_arbitrary_expression_type_inlays(true),
    );
    open_text(&mut enabled_session, uri.clone(), source);
    assert!(
        enabled_session
            .profile_for_uri(&uri)
            .accepted_environment()
            .is_some(),
        "expression-inlay profile was not accepted: {:#?}",
        enabled_session.profile_for_uri(&uri).diagnostics()
    );
    let enabled_hints = inlay_hints(&mut enabled_session, uri);
    let enabled_labels = inlay_hint_string_labels(&enabled_hints);
    let i64_inlays = enabled_labels
        .iter()
        .filter(|label| label.as_str() == ": i64")
        .count();
    assert!(
        i64_inlays >= 3,
        "expected expression inlays for call, binary, and pipe-family expressions; got {enabled_labels:?}"
    );
    assert!(
        i64_inlays < 5,
        "literal and trivial path expression sites should stay suppressed; got {enabled_labels:?}"
    );
    assert!(
        enabled_labels.iter().any(|label| label == ": String"),
        "enabled expression inlays should include non-trivial selector expressions; got {enabled_labels:?}"
    );
    assert!(
        !enabled_labels.iter().any(|label| label == ": Choice"),
        "aggregate literal sites should stay suppressed under the conservative expression inlay policy; got {enabled_labels:?}"
    );
    let unique_sites = enabled_hints
        .iter()
        .filter_map(|hint| {
            let InlayHintLabel::String(label) = &hint.label else {
                return None;
            };
            Some((hint.position.line, hint.position.character, label.as_str()))
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        unique_sites.len(),
        enabled_hints.len(),
        "expression inlays should not duplicate the same label at the same source position: {enabled_hints:?}"
    );
}

#[test]
fn definition_request_returns_effective_style_contributor_ranges() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#202122")
            }
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    alice(rich_text=rich_text_style(ruby=ruby_style(gap=1px))): hi[p]
}
"##;
    let (_project, mut session, uri) =
        accepted_dialogue_session("definition-effective-style", source);

    let response = session.handle_request(Request {
        id: RequestId::from(5),
        method: GotoDefinition::METHOD.to_owned(),
        params: serde_json::json!(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_of(source, "hi[p]"),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        }),
    });
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        response.result.expect("definition response"),
    )
    .expect("definition response decodes");
    let GotoDefinitionResponse::Array(locations) = definition else {
        panic!("expected location array");
    };

    assert!(locations.iter().all(|location| location.uri == uri));
    assert!(
        locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "14px") })
    );
    assert!(
        locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "rgb(\"#202122\")") })
    );
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "1px")
            && location.range.end == position_of(source, "))): hi[p]")
    }));
}

#[test]
fn references_request_returns_all_effective_style_contributors() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#202122")
            }
        }
    }
}

flow opening {
    alice(rich_text.text.color=rgb("#303132")): hi[p]
}
"##;
    let (_project, mut session, uri) =
        accepted_dialogue_session("references-effective-style", source);

    let response = session.handle_request(Request {
        id: RequestId::from(6),
        method: References::METHOD.to_owned(),
        params: serde_json::json!(ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_of(source, "hi[p]"),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: true
            },
        }),
    });
    let locations = serde_json::from_value::<Vec<lsp_types::Location>>(
        response.result.expect("references response"),
    )
    .expect("references response decodes");

    assert!(locations.iter().all(|location| location.uri == uri));
    assert!(
        locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "rgb(\"#202122\")") })
    );
    assert!(
        locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "rgb(\"#303132\")") })
    );
}

#[test]
fn definition_request_on_line_option_returns_matching_style_path() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#101112")
            }
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    alice(rich_text.text.color=rgb("#202122")): hi[p]
}
"##;
    let (_project, mut session, uri) = accepted_dialogue_session("definition-line-option", source);

    let response = session.handle_request(Request {
        id: RequestId::from(7),
        method: GotoDefinition::METHOD.to_owned(),
        params: serde_json::json!(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_of(source, "#202122"),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        }),
    });
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        response.result.expect("definition response"),
    )
    .expect("definition response decodes");
    let GotoDefinitionResponse::Array(locations) = definition else {
        panic!("expected location array");
    };

    assert!(locations.iter().all(|location| location.uri == uri));
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#202122\")")
            && location.range.end == position_of(source, "): hi[p]")
    }));
    assert!(
        !locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "14px") })
    );
}

#[test]
fn references_request_on_line_option_filters_to_matching_style_path() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#101112")
            }
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    alice(rich_text.text.color=rgb("#202122")): hi[p]
}
"##;
    let (_project, mut session, uri) = accepted_dialogue_session("references-line-option", source);

    let response = session.handle_request(Request {
        id: RequestId::from(8),
        method: References::METHOD.to_owned(),
        params: serde_json::json!(ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_of(source, "#202122"),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: true
            },
        }),
    });
    let locations = serde_json::from_value::<Vec<lsp_types::Location>>(
        response.result.expect("references response"),
    )
    .expect("references response decodes");

    assert!(locations.iter().all(|location| location.uri == uri));
    assert!(
        locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "rgb(\"#101112\")") })
    );
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#202122\")")
            && location.range.end == position_of(source, "): hi[p]")
    }));
    assert!(
        !locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "14px") })
    );
}

#[test]
fn hover_on_line_option_filters_effective_style_path() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#101112")
            }
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    alice(rich_text.text.color=rgb("#202122")): hi[p]
}
"##;
    let (_project, mut session, uri) = accepted_dialogue_session("hover-line-option", source);
    let hover = hover_text(&mut session, uri, source, "202122");

    assert!(hover.contains("effective dialogue style `rich_text.text.color` for `alice`"));
    assert!(hover.contains("rich_text.text.color = rgb(\"#202122\")"));
    assert!(hover.contains("rich_text.text.color = rgb(\"#101112\")"));
    assert!(!hover.contains("rich_text.ruby.size = 14px"));
}

#[test]
fn hover_in_child_module_uses_project_global_dialogue_ordinal() {
    let project = TestProject::new("hover-child-dialogue-global-ordinal");
    project.write(
        "arcw.toml",
        r#"schema = 1

[package]
id = "org.arcweft.tests.dialogue.project-cascade"
version = "0.1.0"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
"#,
    );
    let root_source = r##"use self.side.child_helper

pub character root_speaker {}

flow root {
    root_speaker(rich_text.text.color=rgb("#111111")): root[p]
}
"##;
    project.write("src/main.arcw", root_source);
    let child_source = r##"mod side

pub character child_speaker {}

pub fn child_helper() -> Unit {
    ()
}

flow child {
    child_speaker(rich_text.text.color=rgb("#222222")): child[p]
}
"##;
    project.write("src/side.arcw", child_source);

    let root_uri = file_uri(&project.path("src/main.arcw"));
    let uri = file_uri(&project.path("src/side.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, root_uri, root_source);
    open_text(&mut session, uri.clone(), child_source);
    let profile = session.profile_for_uri(&uri);
    assert!(
        profile.accepted_environment().is_some(),
        "child document should belong to an accepted project generation: {:?}",
        profile.diagnostics()
    );

    let hover = hover_text(&mut session, uri, child_source, "222222");
    assert!(hover.contains("for `child_speaker`"), "{hover}");
    assert!(hover.contains("#222222"), "{hover}");
    assert!(!hover.contains("root_speaker"), "{hover}");
    assert!(!hover.contains("#111111"), "{hover}");
}

#[test]
fn hover_on_nested_rich_text_line_option_filters_to_leaf_path() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#101112")
            }
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    alice(rich_text=rich_text_style(ruby=ruby_style(size=11px))): hi[p]
}
"##;
    let (_project, mut session, uri) =
        accepted_dialogue_session("hover-nested-line-option", source);
    let hover = hover_text(&mut session, uri, source, "11px");

    assert!(hover.contains("effective dialogue style `rich_text.ruby.size` for `alice`"));
    assert!(hover.contains("rich_text.ruby.size = 11px"));
    assert!(hover.contains("rich_text.ruby.size = 14px"));
    assert!(!hover.contains("rich_text.text.color"));
}

#[test]
fn hover_on_inline_rich_text_span_filters_to_leaf_path() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#101112")
            }
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    alice: [.ruby_over ruby_size=11px]|[夢](ゆめ)[/][p]
}
"##;
    let (_project, mut session, uri) = accepted_dialogue_session("hover-inline-span", source);
    let hover = hover_text(&mut session, uri, source, "11px");

    assert!(hover.contains("effective dialogue style `rich_text.ruby.size` for `alice`"));
    assert!(hover.contains("rich_text.ruby.size = 11px (inline_span"));
    assert!(hover.contains("rich_text.ruby.size = 14px"));
    assert!(!hover.contains("rich_text.text.color"));
}

#[test]
fn definition_on_nested_rich_text_line_option_returns_leaf_path_winner() {
    let source = r##"
pub character alice {
    dialogue_style {
        rich_text {
            text {
                color = rgb("#101112")
            }
            ruby {
                size = 14px
            }
        }
    }
}

flow opening {
    alice(rich_text=rich_text_style(ruby=ruby_style(size=11px))): hi[p]
}
"##;
    let (_project, mut session, uri) =
        accepted_dialogue_session("definition-nested-line-option", source);

    let response = session.handle_request(Request {
        id: RequestId::from(9),
        method: GotoDefinition::METHOD.to_owned(),
        params: serde_json::json!(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_of(source, "11px"),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        }),
    });
    let definition = serde_json::from_value::<GotoDefinitionResponse>(
        response.result.expect("definition response"),
    )
    .expect("definition response decodes");
    let GotoDefinitionResponse::Array(locations) = definition else {
        panic!("expected location array");
    };

    assert!(locations.iter().all(|location| location.uri == uri));
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "11px")
            && location.range.end == position_of(source, "))): hi[p]")
    }));
    assert!(
        !locations
            .iter()
            .any(|location| { location.range.start == position_of(source, "rgb(\"#101112\")") })
    );
}

fn accepted_dialogue_session(name: &str, source: &str) -> (TestProject, ArcweftLspSession, Uri) {
    let project = TestProject::new(name);
    project.write(
        "arcw.toml",
        r#"schema = 1

[package]
id = "org.arcweft.tests.dialogue.cascade"
version = "0.1.0"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
"#,
    );
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), source);
    (project, session, uri)
}

fn open_fixture(session: &mut ArcweftLspSession, uri: Uri) {
    open_text(
        session,
        uri,
        "flow @.opening opening {\n    alice: hi[p]\n}\n",
    );
}

pub(super) fn open_text(session: &mut ArcweftLspSession, uri: Uri, text: &str) {
    let open = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "arcweft".to_owned(),
                version: 1,
                text: text.to_owned(),
            },
        },
    );
    session.handle_notification(open).expect("open fixture");
}

fn code_action_by_title<'a>(
    actions: &'a [CodeActionOrCommand],
    title: &str,
) -> Option<&'a lsp_types::CodeAction> {
    actions.iter().find_map(|action| match action {
        CodeActionOrCommand::CodeAction(action) if action.title.contains(title) => Some(action),
        CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
    })
}

fn workspace_edit_replacements(action: &lsp_types::CodeAction) -> Vec<String> {
    let Some(edit) = action.edit.as_ref() else {
        return Vec::new();
    };
    edit.changes
        .as_ref()
        .into_iter()
        .flat_map(|changes| changes.values())
        .flatten()
        .map(|edit| edit.new_text.clone())
        .chain(
            edit.document_changes
                .as_ref()
                .into_iter()
                .flat_map(|changes| match changes {
                    lsp_types::DocumentChanges::Edits(edits) => edits
                        .iter()
                        .flat_map(|edit| edit.edits.iter())
                        .filter_map(|edit| match edit {
                            lsp_types::OneOf::Left(edit) => Some(edit.new_text.clone()),
                            lsp_types::OneOf::Right(_) => None,
                        })
                        .collect::<Vec<_>>(),
                    lsp_types::DocumentChanges::Operations(_) => Vec::new(),
                }),
        )
        .collect()
}

fn completion_labels(session: &mut ArcweftLspSession, uri: Uri) -> Vec<lsp_types::CompletionItem> {
    let response = session.handle_request(Request {
        id: RequestId::from(1),
        method: Completion::METHOD.to_owned(),
        params: serde_json::json!(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(0, 0),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        }),
    });
    let response = response.result.expect("completion response");
    match serde_json::from_value::<CompletionResponse>(response)
        .expect("completion response decodes")
    {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    }
}

fn quest_adapter_metadata() -> String {
    let mut metadata = base_adapter_metadata();
    metadata.exports.types = vec![AdapterTypeExport {
        name: AdapterTypeName::new("PlayerStats").expect("type name"),
        visibility: ManifestVisibility::Public,
        shape: AdapterTypeShape::Record {
            fields: vec![
                AdapterTypeField {
                    name: FieldName::new("score").expect("field name"),
                    ty: TypeReference::new("i32").expect("type reference"),
                },
                AdapterTypeField {
                    name: FieldName::new("tags").expect("field name"),
                    ty: TypeReference::new("Vec<String>").expect("type reference"),
                },
            ],
        },
    }];
    metadata.exports.functions = vec![AdapterFunctionExport {
        name: FunctionName::new("quest_evaluate").expect("function name"),
        visibility: ManifestVisibility::Public,
        params: vec![AdapterParameter {
            name: FieldName::new("stats").expect("field name"),
            ty: TypeReference::new("PlayerStats").expect("type reference"),
        }],
        return_type: TypeReference::new("String").expect("type reference"),
        purity: FunctionPurity::Pure,
        effects: Vec::new(),
    }];
    encode_adapter_metadata(metadata)
}

fn hover_text(session: &mut ArcweftLspSession, uri: Uri, source: &str, needle: &str) -> String {
    let response = session.handle_request(Request {
        id: RequestId::from(2),
        method: HoverRequest::METHOD.to_owned(),
        params: serde_json::json!(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: position_of(source, needle),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        }),
    });
    let hover =
        serde_json::from_value::<lsp_types::Hover>(response.result.expect("hover response"))
            .expect("hover response decodes");
    match hover.contents {
        lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(text)) => text,
        other => panic!("unexpected hover contents: {other:?}"),
    }
}

fn native_signature_response(
    session: &Arc<RwLock<ArcweftLspSession>>,
    runtime: &SignatureRequestRuntime,
    client: &Connection,
    request_id: i32,
    uri: Uri,
    position: Position,
) -> Response {
    let id = RequestId::from(request_id);
    let prepared = session
        .read()
        .expect("session read")
        .prepare_signature_request(
            id.clone(),
            SignatureHelpParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                context: None,
            },
            runtime.registry(),
        )
        .expect("prepared signature request");
    runtime
        .submit(prepared)
        .expect("submitted signature request");
    match client
        .receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("signature response")
    {
        Message::Response(response) => response,
        other => panic!("unexpected signature message: {other:?}"),
    }
}

fn inlay_hint_labels(session: &mut ArcweftLspSession, uri: Uri) -> Vec<String> {
    inlay_hint_string_labels(&inlay_hints(session, uri))
}

fn inlay_hints(session: &mut ArcweftLspSession, uri: Uri) -> Vec<InlayHint> {
    let response = session.handle_request(Request {
        id: RequestId::from(4),
        method: InlayHintRequest::METHOD.to_owned(),
        params: serde_json::json!(InlayHintParams {
            text_document: TextDocumentIdentifier { uri },
            range: Range::new(Position::new(0, 0), Position::new(10, 0)),
            work_done_progress_params: WorkDoneProgressParams::default(),
        }),
    });
    serde_json::from_value::<Vec<InlayHint>>(response.result.expect("inlay hint response"))
        .expect("inlay hint response decodes")
}

fn inlay_hint_string_labels(hints: &[InlayHint]) -> Vec<String> {
    hints
        .iter()
        .filter_map(|hint| match &hint.label {
            InlayHintLabel::String(label) => Some(label.clone()),
            InlayHintLabel::LabelParts(_) => None,
        })
        .collect()
}

pub(super) fn position_of(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).expect("needle in source");
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit_once('\n')
        .map_or(before.len(), |(_, tail)| tail.len());
    Position::new(
        u32::try_from(line).expect("fixture line fits"),
        u32::try_from(character).expect("fixture character fits"),
    )
}

pub(super) fn position_after(source: &str, needle: &str) -> Position {
    let start = source.find(needle).expect("needle in source");
    let end = start + needle.len();
    let before = &source[..end];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit_once('\n')
        .map_or(before.len(), |(_, tail)| tail.len());
    Position::new(
        u32::try_from(line).expect("fixture line fits"),
        u32::try_from(character).expect("fixture character fits"),
    )
}

fn canonical_project_manifest(package: &str, profile_tables: &str) -> String {
    format!(
        "schema = 1\n\n[package]\nid = \"org.arcweft.tests.{package}\"\nversion = \"0.1.0\"\n{profile_tables}"
    )
}

fn base_adapter_metadata() -> AdapterMetadata {
    let mut metadata = serde_json::from_str::<AdapterMetadata>(ADAPTER_METADATA)
        .expect("canonical adapter metadata fixture");
    metadata.requirements.clear();
    metadata.exports.types.clear();
    metadata.exports.functions.clear();
    metadata.exports.activities.clear();
    metadata
}

fn adapter_metadata_with_function(function: &str, parameter: &str, result: &str) -> String {
    let mut metadata = base_adapter_metadata();
    metadata.exports.functions = vec![AdapterFunctionExport {
        name: FunctionName::new(function).expect("function name"),
        visibility: ManifestVisibility::Public,
        params: vec![AdapterParameter {
            name: FieldName::new("value").expect("field name"),
            ty: TypeReference::new(parameter).expect("parameter type"),
        }],
        return_type: TypeReference::new(result).expect("return type"),
        purity: FunctionPurity::Pure,
        effects: Vec::new(),
    }];
    encode_adapter_metadata(metadata)
}

fn encode_adapter_metadata(mut metadata: AdapterMetadata) -> String {
    metadata.abi_hash = metadata.computed_abi_hash().expect("adapter ABI hash");
    metadata.payload_hash = metadata
        .computed_payload_hash()
        .expect("adapter payload hash");
    serde_json::to_string_pretty(&metadata).expect("adapter metadata JSON")
}

fn external_module_project_manifest(
    package: &str,
    import: &str,
    mount: &str,
    metadata_path: &str,
    metadata_source: &str,
) -> String {
    let metadata = serde_json::from_str::<AdapterMetadata>(metadata_source)
        .expect("canonical adapter metadata");
    let family = match &metadata.target {
        AdapterTarget::Rust(_) => "rust",
        AdapterTarget::Wasm(_) => "wasm",
        AdapterTarget::Process(_) => "process",
    };
    let raw_hash = RawDigest::for_bytes(metadata_source.as_bytes());
    format!(
        r#"schema = 1

[package]
id = "org.arcweft.tests.{package}"
version = "0.1.0"

[external-modules.{import}]
mount = "{mount}"
metadata = "{metadata_path}"
metadata-hash = "{raw_hash}"
expected-package = "{}"
expected-version = "{}"
expected-module = "{}"
expected-family = "{family}"
expected-abi-hash = "{}"
visibility = "package"
demand = "required"

[profiles.dev]
kind = "server"
entry = "@entry.server.main"
source = "src/main.arcw"
external-modules = ["{import}"]
"#,
        metadata.package.id, metadata.package.version, metadata.module.id, metadata.abi_hash,
    )
}

pub(super) fn file_uri(path: &Path) -> Uri {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let uri = if normalized
        .as_bytes()
        .get(1)
        .is_some_and(|byte| *byte == b':')
    {
        format!("file:///{normalized}")
    } else {
        format!("file://{normalized}")
    };
    uri.parse().expect("file uri")
}

pub(super) struct TestProject {
    root: PathBuf,
}

impl TestProject {
    pub(super) fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{name}-{unique}"));
        create_dir_all(&root).expect("create test project root");
        Self { root }
    }

    pub(super) fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    pub(super) fn write(&self, path: &str, contents: &str) {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).expect("create parent");
        }
        write(path, contents).expect("write fixture");
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
