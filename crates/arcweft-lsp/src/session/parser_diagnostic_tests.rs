use lsp_server::Notification;
use lsp_types::notification::{DidOpenTextDocument, Notification as _};
use lsp_types::{
    CodeActionContext, CodeActionOrCommand, CodeActionParams, DidOpenTextDocumentParams,
    PartialResultParams, Position, Range, TextDocumentIdentifier, TextDocumentItem, Uri,
    WorkDoneProgressParams,
};

use super::ArcweftLspSession;
use crate::config::LspConfig;

#[test]
fn conditional_recovery_stays_in_the_source_lease_across_lsp_edits() {
    use arcweft_lang_syntax::incremental::ParseStatus;
    use lsp_types::notification::DidChangeTextDocument;
    use lsp_types::{
        DidChangeTextDocumentParams, TextDocumentContentChangeEvent,
        VersionedTextDocumentIdentifier,
    };

    let uri = "file:///conditional-recovery.arcw".parse::<Uri>().unwrap();
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let valid = "pub character alice { display = \"Alice\" }\nfn speak() -> Unit { alice()[|[夢](ゆめ)]; () }\n";
    session
        .handle_notification(Notification::new(
            DidOpenTextDocument::METHOD.to_owned(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "arcweft".into(),
                    version: 1,
                    text: valid.into(),
                },
            },
        ))
        .unwrap();
    let initial = session.documents.get(&uri).unwrap().parsed_source().clone();
    assert_eq!(initial.status(), ParseStatus::Conditional);
    assert!(initial.diagnostics().is_empty());
    assert!(
        initial.syntax_stats().diagnostic_identities() > 0,
        "the rejected candidate's source diagnostics remain retained"
    );

    for (version, text, expected) in [
        (
            2,
            "fn broken() -> Unit { let value = |_ trailing| 0; () }\n",
            ParseStatus::Recovered,
        ),
        (3, valid, ParseStatus::Conditional),
    ] {
        let notifications = session
            .handle_notification(Notification::new(
                DidChangeTextDocument::METHOD.to_owned(),
                DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: text.into(),
                    }],
                },
            ))
            .unwrap();
        let current = session.documents.get(&uri).unwrap().parsed_source();
        assert_eq!(current.status(), expected);
        assert!(!current.is_same_snapshot(&initial));
        assert_eq!(
            current.diagnostics().is_empty(),
            expected == ParseStatus::Conditional
        );
        let published = notifications
            .iter()
            .find(|notification| notification.method == "textDocument/publishDiagnostics")
            .expect("diagnostics publication");
        let diagnostics: lsp_types::PublishDiagnosticsParams =
            serde_json::from_value(published.params.clone()).unwrap();
        let has_pattern_error = diagnostics.diagnostics.iter().any(|diagnostic| matches!(diagnostic.code.as_ref(), Some(lsp_types::NumberOrString::String(code)) if code == "syntax.pattern.unexpected_trailing_input"));
        assert_eq!(has_pattern_error, expected == ParseStatus::Recovered);
    }
    assert_eq!(initial.status(), ParseStatus::Conditional);
    assert!(initial.diagnostics().is_empty());
}

#[test]
fn editless_missing_as_parser_suggestion_does_not_create_a_workspace_edit() {
    let uri = "file:///view.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let open = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "arcweft".to_owned(),
                version: 1,
                text: "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n"
                    .to_owned(),
            },
        },
    );
    session
        .handle_notification(open)
        .expect("open missing-`as` source");

    let actions = session.code_actions(&CodeActionParams {
        text_document: TextDocumentIdentifier { uri },
        range: Range::new(Position::new(1, 21), Position::new(1, 28)),
        context: CodeActionContext::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    });

    assert!(
        actions
            .iter()
            .filter_map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => Some(action),
                CodeActionOrCommand::Command(_) => None,
            })
            .flat_map(workspace_edit_replacements)
            .all(|replacement| replacement != "as "),
        "editless parser suggestion produced an executable insertion: {actions:?}"
    );
}

#[test]
fn project_root_recovery_diagnostic_has_no_executable_code_action() {
    let uri = "file:///bare-flow-item.arcw".parse::<Uri>().expect("uri");
    let mut session = ArcweftLspSession::new(&LspConfig::default());
    let open = Notification::new(
        DidOpenTextDocument::METHOD.to_owned(),
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "arcweft".to_owned(),
                version: 1,
                text: "alice: hello\npub character bob {}\n".to_owned(),
            },
        },
    );
    session
        .handle_notification(open)
        .expect("open bare flow item source");

    let actions = session.code_actions(&CodeActionParams {
        text_document: TextDocumentIdentifier { uri },
        range: Range::new(Position::new(0, 0), Position::new(0, 12)),
        context: CodeActionContext::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    });

    assert!(
        actions.is_empty(),
        "project-root recovery received an executable action: {actions:?}"
    );
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
