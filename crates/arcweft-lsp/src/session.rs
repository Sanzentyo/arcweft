use crate::commands::ArcweftCommand;
use crate::config::LspConfig;
use crate::diagnostics::{DocumentAnalysis, publish_diagnostics};
use crate::documents::{DocumentError, DocumentSnapshot, DocumentStore};
use crate::features;
use crate::positions::PositionEncoding;
use crate::profiles::LspProfile;
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
    pub fn new(config: LspConfig) -> Self {
        Self {
            documents: DocumentStore::default(),
            profile: LspProfile::default_for_runner(config.runner()),
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
                Ok(vec![publish_diagnostics_notification(&snapshot)])
            }
            DidChangeTextDocument::METHOD => {
                let params = decode::<DidChangeTextDocumentParams>(
                    DidChangeTextDocument::METHOD,
                    notification.params,
                )?;
                let snapshot = self.documents.change(params, self.position_encoding)?;
                Ok(vec![publish_diagnostics_notification(&snapshot)])
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
                Ok(self
                    .documents
                    .get(&params.text_document.uri)
                    .map_or_else(Vec::new, |snapshot| {
                        vec![publish_diagnostics_notification(snapshot)]
                    }))
            }
            DidChangeWatchedFiles::METHOD => {
                let _params = decode::<DidChangeWatchedFilesParams>(
                    DidChangeWatchedFiles::METHOD,
                    notification.params,
                )?;
                Ok(Vec::new())
            }
            DidChangeConfiguration::METHOD => {
                let _params = decode::<DidChangeConfigurationParams>(
                    DidChangeConfiguration::METHOD,
                    notification.params,
                )?;
                Ok(Vec::new())
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
                let result =
                    ArcweftCommand::parse(&params.command).map_or(Value::Null, |_| Value::Null);
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
}

fn publish_diagnostics_notification(snapshot: &DocumentSnapshot) -> Notification {
    Notification::new(
        PublishDiagnostics::METHOD.to_owned(),
        publish_diagnostics(snapshot),
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
        ClientCapabilities, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
        TextDocumentContentChangeEvent, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    };

    #[test]
    fn capabilities_advertise_full_sync_and_p0_features() {
        let mut session = ArcweftLspSession::new(LspConfig::default());
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
        let mut session = ArcweftLspSession::new(LspConfig::default());

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
}
