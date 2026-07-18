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

    let actions = session
        .code_actions(&CodeActionParams {
            text_document: TextDocumentIdentifier { uri },
            range: Range::new(Position::new(1, 21), Position::new(1, 28)),
            context: CodeActionContext::default(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .expect("missing-`as` code actions");

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
