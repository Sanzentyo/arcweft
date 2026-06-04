use crate::commands::ArcweftCommand;
use crate::config::LspConfig;
use crate::diagnostics::{DocumentAnalysis, publish_diagnostics};
use crate::documents::{DocumentError, DocumentSnapshot, DocumentStore};
use crate::features;
use crate::positions::PositionEncoding;
use crate::profiles::{LspProfile, LspProfileResolver};
use arcweft_verify_lsp::workspace_edit_from_tooling_edit;
use lsp_server::{ErrorCode, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, DidSaveTextDocument, Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, Completion, ExecuteCommand, HoverRequest, InlayHintRequest,
    Request as LspRequest, SignatureHelpRequest,
};
use lsp_types::{
    CodeActionOrCommand, CodeActionParams, CodeActionResponse, CompletionParams,
    CompletionResponse, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, ExecuteCommandOptions, ExecuteCommandParams, HoverParams,
    HoverProviderCapability, InitializeParams, InlayHintParams, InlayHintServerCapabilities, OneOf,
    ServerCapabilities, SignatureHelpOptions, SignatureHelpParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, WorkDoneProgressOptions,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeSet;
use thiserror::Error;

/// Stateful Sans I/O session used by the stdio transport.
#[derive(Debug)]
pub struct ArcweftLspSession {
    documents: DocumentStore,
    profile: LspProfile,
    profile_resolver: LspProfileResolver,
    position_encoding: PositionEncoding,
    cancelled: BTreeSet<RequestId>,
}

/// Error while processing LSP messages.
#[derive(Debug, Error)]
pub enum SessionError {
    /// LSP params could not be decoded into the expected request type.
    #[error("invalid params for `{method}`: {error}")]
    InvalidParams {
        method: &'static str,
        error: serde_json::Error,
    },
    /// Document synchronization failed.
    #[error(transparent)]
    Document(#[from] DocumentError),
}

impl ArcweftLspSession {
    /// Creates a session before initialize is received.
    pub fn new(config: &LspConfig) -> Self {
        let profile_resolver =
            LspProfileResolver::new(config.runner(), config.profile_id().map(str::to_owned));
        Self {
            documents: DocumentStore::default(),
            profile: LspProfile::default_for_runner(config.runner()),
            profile_resolver,
            position_encoding: PositionEncoding::default(),
            cancelled: BTreeSet::new(),
        }
    }

    /// Records initialize params and returns the server capabilities to publish.
    pub fn initialize(&mut self, params: &InitializeParams) -> ServerCapabilities {
        self.position_encoding = PositionEncoding::negotiate(&params.capabilities);
        self.server_capabilities()
    }

    /// Server capabilities for the active negotiated encoding.
    pub fn server_capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            position_encoding: Some(self.position_encoding.as_lsp_kind()),
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(lsp_types::CompletionOptions {
                trigger_characters: Some(vec![".".to_owned(), "@".to_owned(), ":".to_owned()]),
                ..lsp_types::CompletionOptions::default()
            }),
            signature_help_provider: Some(SignatureHelpOptions {
                trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                retrigger_characters: Some(vec![",".to_owned()]),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            }),
            code_action_provider: Some(lsp_types::CodeActionProviderCapability::Simple(true)),
            execute_command_provider: Some(ExecuteCommandOptions {
                commands: ArcweftCommand::all()
                    .into_iter()
                    .map(|command| command.as_str().to_owned())
                    .collect(),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            }),
            inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                lsp_types::InlayHintOptions {
                    resolve_provider: Some(false),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                },
            ))),
            ..ServerCapabilities::default()
        }
    }

    /// Handles one request and returns a response.
    pub fn handle_request(&mut self, request: Request) -> Response {
        let id = request.id.clone();
        if self.cancelled.remove(&id) {
            return Response::new_err(
                id,
                ErrorCode::RequestCanceled as i32,
                "request was cancelled".to_owned(),
            );
        }
        self.try_handle_request(request).unwrap_or_else(|error| {
            Response::new_err(id, ErrorCode::InvalidParams as i32, error.to_string())
        })
    }

    /// Handles one notification and returns notifications to publish.
    pub fn handle_notification(
        &mut self,
        notification: Notification,
    ) -> Result<Vec<Notification>, SessionError> {
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params = decode::<DidOpenTextDocumentParams>(
                    DidOpenTextDocument::METHOD,
                    notification.params,
                )?;
                let snapshot = self.documents.open(params, self.position_encoding);
                self.refresh_profile_for_uri(snapshot.uri());
                Ok(vec![publish_diagnostics_notification(
                    &snapshot,
                    &self.profile,
                )])
            }
            DidChangeTextDocument::METHOD => {
                let params = decode::<DidChangeTextDocumentParams>(
                    DidChangeTextDocument::METHOD,
                    notification.params,
                )?;
                let snapshot = self.documents.change(params, self.position_encoding)?;
                Ok(vec![publish_diagnostics_notification(
                    &snapshot,
                    &self.profile,
                )])
            }
            DidCloseTextDocument::METHOD => {
                let params = decode::<DidCloseTextDocumentParams>(
                    DidCloseTextDocument::METHOD,
                    notification.params,
                )?;
                self.documents.close(&params.text_document.uri);
                Ok(vec![Notification::new(
                    PublishDiagnostics::METHOD.to_owned(),
                    lsp_types::PublishDiagnosticsParams::new(
                        params.text_document.uri,
                        Vec::new(),
                        None,
                    ),
                )])
            }
            DidSaveTextDocument::METHOD => {
                let params = decode::<DidSaveTextDocumentParams>(
                    DidSaveTextDocument::METHOD,
                    notification.params,
                )?;
                self.refresh_profile_for_uri(&params.text_document.uri);
                Ok(self
                    .documents
                    .get(&params.text_document.uri)
                    .map_or_else(Vec::new, |snapshot| {
                        vec![publish_diagnostics_notification(snapshot, &self.profile)]
                    }))
            }
            DidChangeWatchedFiles::METHOD => {
                let _params = decode::<DidChangeWatchedFilesParams>(
                    DidChangeWatchedFiles::METHOD,
                    notification.params,
                )?;
                Ok(self.refresh_profile_for_open_documents())
            }
            DidChangeConfiguration::METHOD => {
                let _params = decode::<DidChangeConfigurationParams>(
                    DidChangeConfiguration::METHOD,
                    notification.params,
                )?;
                Ok(self.refresh_profile_for_open_documents())
            }
            "$/cancelRequest" => {
                if let Some(id) = notification
                    .params
                    .get("id")
                    .and_then(|value| serde_json::from_value(value.clone()).ok())
                {
                    self.cancelled.insert(id);
                }
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn try_handle_request(&self, request: Request) -> Result<Response, SessionError> {
        match request.method.as_str() {
            Completion::METHOD => {
                let (id, _params) = extract::<CompletionParams>(request, Completion::METHOD)?;
                let items = features::completion::completions(&self.profile);
                Ok(Response::new_ok(id, Some(CompletionResponse::Array(items))))
            }
            HoverRequest::METHOD => {
                let (id, params) = extract::<HoverParams>(request, HoverRequest::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document_position_params.text_document.uri)
                    .and_then(|document| {
                        features::hover::hover(
                            &self.profile,
                            document,
                            params.text_document_position_params.position,
                        )
                    });
                Ok(Response::new_ok(id, result))
            }
            SignatureHelpRequest::METHOD => {
                let (id, params) =
                    extract::<SignatureHelpParams>(request, SignatureHelpRequest::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document_position_params.text_document.uri)
                    .and_then(|document| {
                        features::signature::signature_help(
                            &self.profile,
                            document,
                            params.text_document_position_params.position,
                        )
                    });
                Ok(Response::new_ok(id, result))
            }
            CodeActionRequest::METHOD => {
                let (id, params) = extract::<CodeActionParams>(request, CodeActionRequest::METHOD)?;
                let result = self.code_actions(&params);
                Ok(Response::new_ok(id, result))
            }
            InlayHintRequest::METHOD => {
                let (id, params) = extract::<InlayHintParams>(request, InlayHintRequest::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document.uri)
                    .map(features::inlay::hints);
                Ok(Response::new_ok(id, result))
            }
            ExecuteCommand::METHOD => {
                let (id, params) =
                    extract::<ExecuteCommandParams>(request, ExecuteCommand::METHOD)?;
                let result = self.execute_command(&params);
                Ok(Response::new_ok(id, result))
            }
            _ => Ok(Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported request `{}`", request.method),
            )),
        }
    }

    fn document_for_params(&self, uri: &lsp_types::Uri) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    fn code_actions(&self, params: &CodeActionParams) -> Option<CodeActionResponse> {
        let document = self.document_for_params(&params.text_document.uri)?;
        let analysis =
            DocumentAnalysis::analyze(document.text(), document.line_index().position_encoding());
        let actions = features::actions::actions(&params.text_document.uri, document, &analysis)
            .into_iter()
            .map(CodeActionOrCommand::CodeAction)
            .collect();
        Some(actions)
    }

    fn execute_command(&self, params: &ExecuteCommandParams) -> Value {
        let Some(_command) = ArcweftCommand::parse(&params.command) else {
            return Value::Null;
        };
        let Some((uri, edit)) = command_uri_and_edit(params) else {
            return Value::Null;
        };
        let Some(document) = self.document_for_params(&uri) else {
            return Value::Null;
        };
        serde_json::to_value(workspace_edit_from_tooling_edit(
            &uri,
            &edit,
            document.line_index(),
        ))
        .unwrap_or(Value::Null)
    }

    fn refresh_profile_for_uri(&mut self, uri: &lsp_types::Uri) {
        self.profile = self.profile_resolver.resolve_for_uri(uri);
    }

    fn refresh_profile_for_open_documents(&mut self) -> Vec<Notification> {
        let snapshots = self.documents.snapshots().cloned().collect::<Vec<_>>();
        if let Some(snapshot) = snapshots.first() {
            self.refresh_profile_for_uri(snapshot.uri());
        }
        snapshots
            .iter()
            .map(|snapshot| publish_diagnostics_notification(snapshot, &self.profile))
            .collect()
    }
}

fn command_uri_and_edit(
    params: &ExecuteCommandParams,
) -> Option<(lsp_types::Uri, arcweft_tooling::TextEdit)> {
    let uri = params
        .arguments
        .first()
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse().ok())?;
    let edit = params
        .arguments
        .get(1)
        .and_then(|value| serde_json::from_value(value.clone()).ok())?;
    Some((uri, edit))
}

fn publish_diagnostics_notification(
    snapshot: &DocumentSnapshot,
    profile: &LspProfile,
) -> Notification {
    Notification::new(
        PublishDiagnostics::METHOD.to_owned(),
        publish_diagnostics(snapshot, profile),
    )
}

fn extract<P: DeserializeOwned>(
    request: Request,
    method: &'static str,
) -> Result<(RequestId, P), SessionError> {
    let id = request.id;
    let params = decode(method, request.params)?;
    Ok((id, params))
}

fn decode<P: DeserializeOwned>(method: &'static str, value: Value) -> Result<P, SessionError> {
    serde_json::from_value(value).map_err(|error| SessionError::InvalidParams { method, error })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{
        ClientCapabilities, CodeActionContext, DidChangeTextDocumentParams,
        DidOpenTextDocumentParams, PartialResultParams, Position, Range,
        TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
        TextDocumentPositionParams, Uri, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
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
    fn execute_command_can_return_workspace_edit_from_tooling_edit_argument() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut session = ArcweftLspSession::new(&LspConfig::default());
        open_fixture(&mut session, uri.clone());
        let edit = arcweft_tooling::TextEdit {
            start: 0,
            end: 0,
            replacement: "// generated\n".to_owned(),
        };

        let result = session.execute_command(&ExecuteCommandParams {
            command: ArcweftCommand::ExpandSugar.as_str().to_owned(),
            arguments: vec![serde_json::json!(uri.to_string()), serde_json::json!(edit)],
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
        let completions = match serde_json::from_value::<CompletionResponse>(response)
            .expect("completion response decodes")
        {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        };
        assert!(completions.iter().any(|item| item.label == "custom.echo"));
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
}
