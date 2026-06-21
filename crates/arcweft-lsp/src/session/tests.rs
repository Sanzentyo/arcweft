use super::*;
use arcweft_rust_abi::{
    ArcweftRustField, ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage,
    ArcweftRustParam, ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind,
    ArcweftRustTypeRef,
};
use lsp_types::{
    ClientCapabilities, CodeActionContext, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidOpenTextDocumentParams, GotoDefinitionResponse, InlayHint,
    InlayHintLabel, PartialResultParams, Position, Range, ReferenceContext, SignatureHelp,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
    WorkspaceClientCapabilities, WorkspaceEditClientCapabilities,
};
use std::{
    fs::{create_dir_all, write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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
fn code_actions_return_workspace_edits() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_fixture(&mut session, uri.clone());

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri },
            range: Range::new(Position::new(0, 0), Position::new(10, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    assert!(actions.iter().any(|action| {
        matches!(action, CodeActionOrCommand::CodeAction(action) if action.edit.is_some())
    }));
}

#[test]
fn code_actions_add_missing_effect_declaration() {
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

    let action = code_action_by_title(&actions, "Add missing effect declaration")
        .expect("missing effect quickfix exists");
    assert!(workspace_edit_replacements(action).contains(&"effects { fs.read }".to_owned()));
}

#[test]
fn code_actions_remove_unused_effect_declaration() {
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

    let action = code_action_by_title(&actions, "Remove unused effect declaration")
        .expect("unused effect quickfix exists");
    assert!(workspace_edit_replacements(action).contains(&"effects { }".to_owned()));
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
fn code_actions_expand_sugar_respects_decl_identity_attributes() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = "#[generated]\nflow @flow.generated generated {\n}\n#[allow(style::redundant_decl_identity)]\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\nflow @flow.opening opening {\n}\nflow @flow.opening start {\n}\n";
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
            CodeActionOrCommand::CodeAction(action) if action.title == "Expand Arcweft sugar" => {
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
fn code_actions_expand_sugar_respects_source_allow_decl_identity_attribute() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = "#![allow(style::redundant_decl_identity)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
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
            CodeActionOrCommand::CodeAction(action) if action.title == "Expand Arcweft sugar" => {
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
fn code_actions_expand_sugar_nests_dotted_dialogue_defaults() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = "pub dialogue defaults @dialogue.defaults {\n    rich_text.ruby.size = 14px\n}\n";
    open_text(&mut session, uri.clone(), source);

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range::new(Position::new(0, 0), Position::new(3, 0)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("open document actions");

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action) if action.title == "Expand Arcweft sugar" => {
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
    assert!(rewritten.contains("rich_text {\n        ruby {\n            size = 14px"));
    assert!(!rewritten.contains("rich_text.ruby.size"));
}

#[test]
fn code_actions_extract_active_style_contributor_to_line_options() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

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
fn code_actions_extract_active_style_contributor_to_character_dialogue_style() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

pub character alice {}

flow opening {
    alice: |[夢](ゆめ)[p]
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

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title
                    == "Extract `rich_text.ruby.size` override to character dialogue_style" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("character extraction action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text,
        "\n    dialogue_style {\n        rich_text {\n            ruby {\n                size = 14px\n            }\n        }\n    }"
    );
}

#[test]
fn code_actions_extract_active_style_contributor_to_textbox_theme() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r"
pub textbox @textbox.phone PhoneBox {}

flow opening {
    alice(window=@textbox.phone, rich_text=rich_text_style(ruby=ruby_style(size=14px))): |[夢](ゆめ)[p]
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

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Extract `rich_text.ruby.size` override to textbox theme" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("textbox theme extraction action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text,
        "\n    rich_text {\n        ruby {\n            size = 14px\n        }\n    }"
    );
}

#[test]
fn code_actions_extract_active_style_contributor_to_speaker_preset() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
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

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Extract `rich_text.ruby.size` override to speaker preset" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("speaker preset extraction action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, ", rich_text.ruby.size=14px");
}

#[test]
fn code_actions_extract_active_style_contributor_to_dialogue_defaults() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r##"
pub dialogue defaults @dialogue.defaults {
}

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

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title
                    == "Extract `rich_text.text.color` override to dialogue defaults" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("dialogue defaults extraction action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text,
        "    rich_text {\n        text {\n            color = rgb(\"#202122\")\n        }\n    }\n"
    );
}

#[test]
fn code_actions_extract_to_profile_selected_dialogue_defaults() {
    let project = TestProject::new("lsp-session-dialogue-defaults-extract-profile");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
}

pub dialogue defaults @dialogue.defaults.mobile {
}

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
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.defaults.mobile"
"#,
    );
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
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

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title
                    == "Extract `rich_text.text.color` override to dialogue defaults" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("dialogue defaults extraction action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");
    let mobile_close = position_of(source, "}\n\npub character alice");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range.start, mobile_close);
    assert_eq!(
        edits[0].new_text,
        "    rich_text {\n        text {\n            color = rgb(\"#202122\")\n        }\n    }\n"
    );
}

#[test]
fn code_actions_extract_rich_text_contributor_to_nested_dialogue_defaults() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let source = r"
pub dialogue defaults @dialogue.defaults {
}

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
    alice: |[夢](ゆめ)[p]
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

    let action = actions
        .iter()
        .find_map(|action| match action {
            CodeActionOrCommand::CodeAction(action)
                if action.title
                    == "Extract `rich_text.ruby.size` override to dialogue defaults" =>
            {
                Some(action)
            }
            CodeActionOrCommand::CodeAction(_) | CodeActionOrCommand::Command(_) => None,
        })
        .expect("dialogue defaults extraction action");
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("workspace edit");

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text,
        "    rich_text {\n        ruby {\n            size = 14px\n        }\n    }\n"
    );
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
fn did_open_refreshes_project_profile_for_completion() {
    let project = TestProject::new("lsp-session-profile");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "custom-echo"
adapter_manifests = ["adapters/custom-echo.toml"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write(
        "adapters/custom-echo.toml",
        r#"
schema_version = 1
id = "custom-echo"
display_name = "Custom Echo"

[[functions]]
name = "custom.echo"
return_type = "String"
params = [{ name = "value", ty = "String" }]
"#,
    );
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
fn hover_uses_profile_selected_dialogue_defaults() {
    let project = TestProject::new("lsp-session-dialogue-defaults-profile");
    let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

pub dialogue defaults @dialogue.defaults.mobile {
    rich_text {
        ruby {
            size = 10px
        }
    }
}

flow opening {
    alice: |[夢](ゆめ)[p]
}
";
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.defaults.mobile"
"#,
    );
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), source);

    let hover = hover_text(&mut session, uri, source, "夢");

    assert!(hover.contains("rich_text.ruby.size = 10px"));
    assert!(!hover.contains("rich_text.ruby.size = 14px"));
}

#[test]
fn definition_includes_profile_selected_dialogue_defaults_manifest_location() {
    let project = TestProject::new("lsp-session-dialogue-defaults-definition");
    let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

pub dialogue defaults @dialogue.defaults.mobile {
    rich_text {
        ruby {
            size = 10px
        }
    }
}

flow opening {
    alice: |[夢](ゆめ)[p]
}
";
    let manifest = r#"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.defaults.mobile"
"#;
    project.write("arcw.toml", manifest);
    project.write("src/main.arcw", source);
    let source_uri = file_uri(&project.path("src/main.arcw"));
    let manifest_uri = file_uri(&project.path("arcw.toml"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, source_uri.clone(), source);

    let response = session.handle_request(Request {
        id: RequestId::from(10),
        method: GotoDefinition::METHOD.to_owned(),
        params: serde_json::json!(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: source_uri.clone()
                },
                position: position_of(source, "夢"),
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

    assert!(locations.iter().any(|location| {
        location.uri == source_uri
            && location.range.start == position_of(source, "10px")
            && location.range.end == position_of(source, "\n        }\n    }\n}\n\nflow")
    }));
    assert!(locations.iter().any(|location| {
        location.uri == manifest_uri
            && location.range.start == position_of(manifest, "dialogue.defaults.mobile")
            && location.range.end == position_after(manifest, "dialogue.defaults.mobile")
    }));
}

#[test]
fn references_include_profile_selected_dialogue_defaults_manifest_location() {
    let project = TestProject::new("lsp-session-dialogue-defaults-references");
    let source = r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

pub dialogue defaults @dialogue.defaults.mobile {
    rich_text {
        ruby {
            size = 10px
        }
    }
}

flow opening {
    alice: |[夢](ゆめ)[p]
}
";
    let manifest = r#"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.defaults.mobile"
"#;
    project.write("arcw.toml", manifest);
    project.write("src/main.arcw", source);
    let source_uri = file_uri(&project.path("src/main.arcw"));
    let manifest_uri = file_uri(&project.path("arcw.toml"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, source_uri.clone(), source);

    let response = session.handle_request(Request {
        id: RequestId::from(11),
        method: References::METHOD.to_owned(),
        params: serde_json::json!(ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: source_uri.clone()
                },
                position: position_of(source, "夢"),
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

    assert!(locations.iter().any(|location| {
        location.uri == source_uri
            && location.range.start == position_of(source, "10px")
            && location.range.end == position_of(source, "\n        }\n    }\n}\n\nflow")
    }));
    assert!(locations.iter().any(|location| {
        location.uri == manifest_uri
            && location.range.start == position_of(manifest, "dialogue.defaults.mobile")
            && location.range.end == position_after(manifest, "dialogue.defaults.mobile")
    }));
}

#[test]
fn completions_use_document_scoped_profiles() {
    let alpha = TestProject::new("lsp-session-alpha");
    alpha.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "alpha"
adapter_manifests = ["adapters/alpha.toml"]
"#,
    );
    alpha.write("src/main.arcw", "flow @.main main {}\n");
    alpha.write(
        "adapters/alpha.toml",
        adapter_manifest("alpha", "alpha.call").as_str(),
    );
    let beta = TestProject::new("lsp-session-beta");
    beta.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "beta"
adapter_manifests = ["adapters/beta.toml"]
"#,
    );
    beta.write("src/main.arcw", "flow @.main main {}\n");
    beta.write(
        "adapters/beta.toml",
        adapter_manifest("beta", "beta.call").as_str(),
    );
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
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "custom"
adapter_manifests = ["adapters/custom.toml"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write(
        "adapters/custom.toml",
        adapter_manifest("custom", "custom.before").as_str(),
    );
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), "flow @.main main {}\n");
    assert!(
        completion_labels(&mut session, uri.clone())
            .iter()
            .any(|item| item.label == "custom.before")
    );

    project.write(
        "adapters/custom.toml",
        adapter_manifest("custom", "custom.after").as_str(),
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
fn watched_file_change_refreshes_rust_metadata() {
    let project = TestProject::new("lsp-session-rust-watch-refresh");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "sans-io"
rust_metadata = ["target/arcweft/quest.json"]
"#,
    );
    project.write("src/main.arcw", "flow @.main main {}\n");
    project.write("target/arcweft/quest.json", "{ not json");
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), "flow @.main main {}\n");
    assert!(
        !completion_labels(&mut session, uri.clone())
            .iter()
            .any(|item| item.label == "quest.evaluate")
    );

    project.write(
        "target/arcweft/quest.json",
        &quest_rust_manifest()
            .to_json_pretty()
            .expect("metadata json"),
    );
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
            .any(|item| item.label == "quest.evaluate")
    );
}

#[test]
fn session_reads_rust_metadata_for_completion_and_hover() {
    let project = TestProject::new("lsp-session-rust-metadata");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "quest"
adapter_manifests = ["adapters/quest.toml"]
rust_metadata = ["target/arcweft/quest.json"]
"#,
    );
    project.write(
        "adapters/quest.toml",
        adapter_manifest("quest", "quest.echo").as_str(),
    );
    project.write(
        "target/arcweft/quest.json",
        &quest_rust_manifest()
            .to_json_pretty()
            .expect("metadata json"),
    );
    let source =
        "flow @.main main {\n    let result = quest.evaluate\n    let ty = PlayerStats\n}\n";
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), source);

    let completions = completion_labels(&mut session, uri.clone());
    let player_stats = completions
        .iter()
        .find(|item| item.label == "PlayerStats")
        .expect("PlayerStats completion");
    assert!(player_stats.detail.as_deref().is_some_and(|detail| {
        detail.contains("struct PlayerStats") && detail.contains("tags: Vec<String>")
    }));
    let evaluate = completions
        .iter()
        .find(|item| item.label == "quest.evaluate")
        .expect("quest.evaluate completion");
    assert!(
        evaluate
            .detail
            .as_deref()
            .is_some_and(|detail| detail == "quest.evaluate(stats: PlayerStats) -> String")
    );

    let hover = hover_text(&mut session, uri, source, "PlayerStats");
    assert!(hover.contains("struct PlayerStats"));
    assert!(hover.contains("Package: quest_logic"));
}

#[test]
fn signature_help_uses_document_scoped_rust_metadata() {
    let project = TestProject::new("lsp-session-signature-rust-metadata");
    project.write(
        "arcw.toml",
        r#"
[profiles.dev]
kind = "server"
source = "src/main.arcw"
adapter = "quest"
adapter_manifests = ["adapters/quest.toml"]
rust_metadata = ["target/arcweft/quest.json"]
"#,
    );
    project.write(
        "adapters/quest.toml",
        adapter_manifest("quest", "quest.echo").as_str(),
    );
    project.write(
        "target/arcweft/quest.json",
        &quest_rust_manifest()
            .to_json_pretty()
            .expect("metadata json"),
    );
    let source = "flow @.main main {\n    let result = quest.evaluate\n}\n";
    project.write("src/main.arcw", source);
    let uri = file_uri(&project.path("src/main.arcw"));
    let mut session = ArcweftLspSession::new(&LspConfig::default().with_profile_id("dev"));
    open_text(&mut session, uri.clone(), source);

    let signature = signature_help(&mut session, uri, source, "quest.evaluate");

    let first = signature.signatures.first().expect("signature item");
    assert_eq!(first.label, "quest.evaluate(stats: PlayerStats) -> String");
    assert_eq!(first.parameters.as_ref().expect("parameters").len(), 1);
}

#[test]
fn inlay_hint_request_uses_document_line_index() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_fixture(&mut session, uri.clone());

    let labels = inlay_hint_labels(&mut session, uri);

    assert!(labels.iter().any(|label| label == "@flow.opening"));
    assert!(
        labels
            .iter()
            .any(|label| label.contains("id=@say.opening.alice.001"))
    );
}

#[test]
fn definition_request_returns_effective_style_contributor_ranges() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

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
    let alice_side = alice(rich_text=rich_text_style(ruby=ruby_style(gap=1px)))
    alice_side: hi[p]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

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
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "14px")
            && location.range.end == position_of(source, "\n        }\n")
    }));
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#202122\")")
            && location.range.end
                == position_of(source, "\n            }\n        }\n    }\n}\n\nflow")
    }));
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rich_text_style")
            && location.range.end == position_of(source, ")\n    alice_side")
    }));
}

#[test]
fn references_request_returns_all_effective_style_contributors() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        text {
            color = rgb("#101112")
        }
    }
}

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
    @<character.alice>.say[hi[p]]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

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
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#101112\")")
            && location.range.end == position_of(source, "\n        }\n    }\n}\n\npub character")
    }));
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#202122\")")
            && location.range.end
                == position_of(source, "\n            }\n        }\n    }\n}\n\nflow")
    }));
}

#[test]
fn definition_request_on_line_option_returns_matching_style_path() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        text {
            color = rgb("#101112")
        }
        ruby {
            size = 14px
        }
    }
}

flow opening {
    alice(rich_text.text.color=rgb("#202122")): hi[p]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

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
    assert!(!locations.iter().any(|location| {
        location.range.start == position_of(source, "14px")
            && location.range.end == position_of(source, "\n        }\n")
    }));
}

#[test]
fn references_request_on_line_option_filters_to_matching_style_path() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        text {
            color = rgb("#101112")
        }
        ruby {
            size = 14px
        }
    }
}

flow opening {
    alice(rich_text.text.color=rgb("#202122")): hi[p]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

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
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#101112\")")
            && location.range.end == position_of(source, "\n        }\n        ruby")
    }));
    assert!(locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#202122\")")
            && location.range.end == position_of(source, "): hi[p]")
    }));
    assert!(!locations.iter().any(|location| {
        location.range.start == position_of(source, "14px")
            && location.range.end == position_of(source, "\n        }\n")
    }));
}

#[test]
fn hover_on_line_option_filters_effective_style_path() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        text {
            color = rgb("#101112")
        }
        ruby {
            size = 14px
        }
    }
}

flow opening {
    alice(rich_text.text.color=rgb("#202122")): hi[p]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);
    let hover = hover_text(&mut session, uri, source, "202122");

    assert!(hover.contains("effective dialogue style `rich_text.text.color` for `alice`"));
    assert!(hover.contains("rich_text.text.color = rgb(\"#202122\")"));
    assert!(hover.contains("rich_text.text.color = rgb(\"#101112\")"));
    assert!(!hover.contains("rich_text.ruby.size = 14px"));
}

#[test]
fn hover_on_nested_rich_text_line_option_filters_to_leaf_path() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        text {
            color = rgb("#101112")
        }
        ruby {
            size = 14px
        }
    }
}

flow opening {
    alice(rich_text=rich_text_style(ruby=ruby_style(size=11px))): hi[p]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);
    let hover = hover_text(&mut session, uri, source, "11px");

    assert!(hover.contains("effective dialogue style `rich_text.ruby.size` for `alice`"));
    assert!(hover.contains("rich_text.ruby.size = 11px"));
    assert!(hover.contains("rich_text.ruby.size = 14px"));
    assert!(!hover.contains("rich_text.text.color"));
}

#[test]
fn hover_on_inline_rich_text_span_filters_to_leaf_path() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        text {
            color = rgb("#101112")
        }
        ruby {
            size = 14px
        }
    }
}

flow opening {
    alice: [.ruby_over ruby_size=11px]|[夢](ゆめ)[/][p]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);
    let hover = hover_text(&mut session, uri, source, "11px");

    assert!(hover.contains("effective dialogue style `rich_text.ruby.size` for `alice`"));
    assert!(hover.contains("rich_text.ruby.size = 11px (inline_span"));
    assert!(hover.contains("rich_text.ruby.size = 14px"));
    assert!(!hover.contains("rich_text.text.color"));
}

#[test]
fn definition_on_nested_rich_text_line_option_returns_leaf_path_winner() {
    let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
    let source = r##"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        text {
            color = rgb("#101112")
        }
        ruby {
            size = 14px
        }
    }
}

flow opening {
    alice(rich_text=rich_text_style(ruby=ruby_style(size=11px))): hi[p]
}
"##;
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    open_text(&mut session, uri.clone(), source);

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
    assert!(!locations.iter().any(|location| {
        location.range.start == position_of(source, "rgb(\"#101112\")")
            && location.range.end == position_of(source, "\n        }\n        ruby")
    }));
}

fn open_fixture(session: &mut ArcweftLspSession, uri: Uri) {
    open_text(
        session,
        uri,
        "flow @.opening opening {\n    alice: hi[p]\n}\n",
    );
}

fn open_text(session: &mut ArcweftLspSession, uri: Uri, text: &str) {
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

fn quest_rust_manifest() -> ArcweftRustManifest {
    ArcweftRustManifest::new(ArcweftRustPackage {
        name: "quest_logic".to_owned(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_type(ArcweftRustTypeDecl {
        name: "PlayerStats".to_owned(),
        rust_path: "quest_logic::PlayerStats".to_owned(),
        kind: ArcweftRustTypeKind::Struct {
            fields: vec![
                ArcweftRustField {
                    name: "score".to_owned(),
                    ty: ArcweftRustTypeRef::I32,
                },
                ArcweftRustField {
                    name: "tags".to_owned(),
                    ty: ArcweftRustTypeRef::Vec {
                        item: Box::new(ArcweftRustTypeRef::String),
                    },
                },
            ],
        },
    })
    .with_function(ArcweftRustFunction {
        name: "quest.evaluate".to_owned(),
        rust_path: "quest_logic::evaluate".to_owned(),
        params: vec![ArcweftRustParam {
            name: "stats".to_owned(),
            ty: ArcweftRustTypeRef::Named {
                name: "PlayerStats".to_owned(),
            },
        }],
        return_type: ArcweftRustTypeRef::String,
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    })
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

fn signature_help(
    session: &mut ArcweftLspSession,
    uri: Uri,
    source: &str,
    needle: &str,
) -> SignatureHelp {
    let response = session.handle_request(Request {
        id: RequestId::from(3),
        method: SignatureHelpRequest::METHOD.to_owned(),
        params: serde_json::json!(SignatureHelpParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: position_of(source, needle),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            context: None,
        }),
    });
    serde_json::from_value(response.result.expect("signature help response"))
        .expect("signature help response decodes")
}

fn inlay_hint_labels(session: &mut ArcweftLspSession, uri: Uri) -> Vec<String> {
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
        .into_iter()
        .filter_map(|hint| match hint.label {
            InlayHintLabel::String(label) => Some(label),
            InlayHintLabel::LabelParts(_) => None,
        })
        .collect()
}

fn position_of(source: &str, needle: &str) -> Position {
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

fn position_after(source: &str, needle: &str) -> Position {
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

fn adapter_manifest(id: &str, function: &str) -> String {
    let mut param = toml::Table::new();
    param.insert("name".to_owned(), toml::Value::String("value".to_owned()));
    param.insert("ty".to_owned(), toml::Value::String("String".to_owned()));

    let mut function_entry = toml::Table::new();
    function_entry.insert("name".to_owned(), toml::Value::String(function.to_owned()));
    function_entry.insert(
        "return_type".to_owned(),
        toml::Value::String("String".to_owned()),
    );
    function_entry.insert(
        "params".to_owned(),
        toml::Value::Array(vec![toml::Value::Table(param)]),
    );

    let mut manifest = toml::Table::new();
    manifest.insert(
        "schema_version".to_owned(),
        toml::Value::Integer(i64::from(
            arcweft_adapter_context::codec::ADAPTER_MANIFEST_SCHEMA_VERSION,
        )),
    );
    manifest.insert("id".to_owned(), toml::Value::String(id.to_owned()));
    manifest.insert(
        "display_name".to_owned(),
        toml::Value::String(id.to_owned()),
    );
    manifest.insert(
        "functions".to_owned(),
        toml::Value::Array(vec![toml::Value::Table(function_entry)]),
    );
    toml::to_string(&toml::Value::Table(manifest)).expect("adapter manifest TOML")
}

fn file_uri(path: &Path) -> Uri {
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

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{name}-{unique}"));
        create_dir_all(&root).expect("create test project root");
        Self { root }
    }

    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    fn write(&self, path: &str, contents: &str) {
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
