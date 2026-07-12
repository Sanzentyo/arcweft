use crate::commands::ArcweftCommand;
use crate::config::LspConfig;
use crate::custom::ArcweftCustomRequest;
use crate::diagnostics::{DocumentAnalysis, publish_diagnostics};
use crate::documents::{DocumentError, DocumentSnapshot, DocumentStore};
use crate::features;
use crate::positions::PositionEncoding;
use crate::profiles::{LspProfile, LspProfileResolver};
use crate::repl_command::{LspReplCommandExecutor, LspReplCommandRequest, LspReplCommandResponse};
use arcweft_verify_lsp::workspace_edit_from_tooling_edit;
use lsp_server::{ErrorCode, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, DidSaveTextDocument, Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, Completion, ExecuteCommand, GotoDefinition, HoverRequest, InlayHintRequest,
    References, Request as LspRequest, SignatureHelpRequest,
};
use lsp_types::{
    CodeActionOrCommand, CodeActionParams, CodeActionResponse, CompletionParams,
    CompletionResponse, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, ExecuteCommandOptions, ExecuteCommandParams, GotoDefinitionParams,
    HoverParams, HoverProviderCapability, InitializeParams, InlayHintParams,
    InlayHintServerCapabilities, OneOf, OptionalVersionedTextDocumentIdentifier, ReferenceParams,
    ServerCapabilities, SignatureHelpOptions, SignatureHelpParams, TextDocumentEdit,
    TextDocumentSyncCapability, TextDocumentSyncKind, WorkDoneProgressOptions, WorkspaceEdit,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Stateful Sans I/O session used by the stdio transport.
#[derive(Debug)]
pub struct ArcweftLspSession {
    documents: DocumentStore,
    default_profile: LspProfile,
    profiles_by_uri: BTreeMap<String, LspProfile>,
    profile_resolver: LspProfileResolver,
    workspace_edit_policy: WorkspaceEditPolicy,
    position_encoding: PositionEncoding,
    cancelled: BTreeSet<RequestId>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WorkspaceEditPolicy {
    document_changes: bool,
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
            LspProfileResolver::new(config.runner(), config.profile_id().map(str::to_owned))
                .with_arbitrary_expression_type_inlays(config.arbitrary_expression_type_inlays());
        let default_profile = profile_resolver.default_profile();
        Self {
            documents: DocumentStore::default(),
            default_profile,
            profiles_by_uri: BTreeMap::new(),
            profile_resolver,
            workspace_edit_policy: WorkspaceEditPolicy::default(),
            position_encoding: PositionEncoding::default(),
            cancelled: BTreeSet::new(),
        }
    }

    /// Records initialize params and returns the server capabilities to publish.
    pub fn initialize(&mut self, params: &InitializeParams) -> ServerCapabilities {
        self.position_encoding = PositionEncoding::negotiate(&params.capabilities);
        self.workspace_edit_policy = WorkspaceEditPolicy::from_initialize(params);
        self.server_capabilities()
    }

    /// Server capabilities for the active negotiated encoding.
    pub fn server_capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            position_encoding: Some(self.position_encoding.as_lsp_kind()),
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
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
        self.handle_request_with_repl_executor(request, None)
    }

    /// Handles one request with an optional borrowed REPL command executor.
    pub fn handle_request_with_repl_executor(
        &mut self,
        request: Request,
        repl: Option<&mut dyn LspReplCommandExecutor>,
    ) -> Response {
        let id = request.id.clone();
        if self.cancelled.remove(&id) {
            return Response::new_err(
                id,
                ErrorCode::RequestCanceled as i32,
                "request was cancelled".to_owned(),
            );
        }
        self.try_handle_request(request, repl)
            .unwrap_or_else(|error| {
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
                Ok(vec![self.publish_diagnostics_notification(&snapshot)])
            }
            DidChangeTextDocument::METHOD => {
                let params = decode::<DidChangeTextDocumentParams>(
                    DidChangeTextDocument::METHOD,
                    notification.params,
                )?;
                let snapshot = self.documents.change(params, self.position_encoding)?;
                Ok(vec![self.publish_diagnostics_notification(&snapshot)])
            }
            DidCloseTextDocument::METHOD => {
                let params = decode::<DidCloseTextDocumentParams>(
                    DidCloseTextDocument::METHOD,
                    notification.params,
                )?;
                self.profiles_by_uri
                    .remove(&params.text_document.uri.to_string());
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
                        vec![self.publish_diagnostics_notification(snapshot)]
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

    fn try_handle_request(
        &self,
        request: Request,
        repl: Option<&mut dyn LspReplCommandExecutor>,
    ) -> Result<Response, SessionError> {
        match request.method.as_str() {
            Completion::METHOD => {
                let (id, params) = extract::<CompletionParams>(request, Completion::METHOD)?;
                let profile =
                    self.profile_for_uri(&params.text_document_position.text_document.uri);
                let document =
                    self.document_for_params(&params.text_document_position.text_document.uri);
                let items = features::completion::completions(profile, document);
                Ok(Response::new_ok(id, Some(CompletionResponse::Array(items))))
            }
            HoverRequest::METHOD => {
                let (id, params) = extract::<HoverParams>(request, HoverRequest::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document_position_params.text_document.uri)
                    .and_then(|document| {
                        let profile = self.profile_for_uri(document.uri());
                        features::hover::hover(
                            profile,
                            document,
                            params.text_document_position_params.position,
                        )
                    });
                Ok(Response::new_ok(id, result))
            }
            GotoDefinition::METHOD => {
                let (id, params) =
                    extract::<GotoDefinitionParams>(request, GotoDefinition::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document_position_params.text_document.uri)
                    .and_then(|document| {
                        let profile = self.profile_for_uri(document.uri());
                        features::definition::definition(
                            profile,
                            &params.text_document_position_params.text_document.uri,
                            document,
                            params.text_document_position_params.position,
                        )
                    });
                Ok(Response::new_ok(id, result))
            }
            References::METHOD => {
                let (id, params) = extract::<ReferenceParams>(request, References::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document_position.text_document.uri)
                    .map(|document| {
                        let profile = self.profile_for_uri(document.uri());
                        features::references::references(
                            profile,
                            &params.text_document_position.text_document.uri,
                            document,
                            params.text_document_position.position,
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
                        let profile = self.profile_for_uri(document.uri());
                        features::signature::signature_help(
                            profile,
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
                    .map(|document| {
                        features::inlay::hints(self.profile_for_uri(document.uri()), document)
                    });
                Ok(Response::new_ok(id, result))
            }
            method if method == ArcweftCustomRequest::ReplCommand.as_str() => {
                Self::handle_repl_command_request(request, repl)
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

    fn handle_repl_command_request(
        request: Request,
        repl: Option<&mut dyn LspReplCommandExecutor>,
    ) -> Result<Response, SessionError> {
        let (id, params) =
            extract::<LspReplCommandRequest>(request, ArcweftCustomRequest::ReplCommand.as_str())?;
        let result = match repl {
            Some(executor) => executor.execute_repl_command(params),
            None => LspReplCommandResponse::host_unavailable(&params),
        };
        Ok(Response::new_ok(id, result))
    }

    fn document_for_params(&self, uri: &lsp_types::Uri) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    fn code_actions(&self, params: &CodeActionParams) -> Option<CodeActionResponse> {
        let document = self.document_for_params(&params.text_document.uri)?;
        let analysis = DocumentAnalysis::analyze(
            document.text(),
            document.line_index().position_encoding(),
            self.profile_for_uri(document.uri()),
        );
        let actions = features::actions::actions(
            self.profile_for_uri(document.uri()),
            &params.text_document.uri,
            document,
            &analysis,
            params.range.start,
        )
        .into_iter()
        .map(|mut action| {
            action.edit = action.edit.map(|edit| {
                self.workspace_edit_policy
                    .normalize(edit, document.uri(), document.version())
            });
            CodeActionOrCommand::CodeAction(action)
        })
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
        let edit = workspace_edit_from_tooling_edit(&uri, &edit, document.line_index());
        let edit = self
            .workspace_edit_policy
            .normalize(edit, document.uri(), document.version());
        serde_json::to_value(edit).unwrap_or(Value::Null)
    }

    fn refresh_profile_for_uri(&mut self, uri: &lsp_types::Uri) {
        self.profiles_by_uri
            .insert(uri.to_string(), self.profile_resolver.resolve_for_uri(uri));
    }

    fn refresh_profile_for_open_documents(&mut self) -> Vec<Notification> {
        let snapshots = self.documents.snapshots().cloned().collect::<Vec<_>>();
        for snapshot in &snapshots {
            self.refresh_profile_for_uri(snapshot.uri());
        }
        snapshots
            .iter()
            .map(|snapshot| self.publish_diagnostics_notification(snapshot))
            .collect()
    }

    fn profile_for_uri(&self, uri: &lsp_types::Uri) -> &LspProfile {
        self.profiles_by_uri
            .get(&uri.to_string())
            .unwrap_or(&self.default_profile)
    }

    fn publish_diagnostics_notification(&self, snapshot: &DocumentSnapshot) -> Notification {
        Notification::new(
            PublishDiagnostics::METHOD.to_owned(),
            publish_diagnostics(snapshot, self.profile_for_uri(snapshot.uri())),
        )
    }
}

fn command_uri_and_edit(
    params: &ExecuteCommandParams,
) -> Option<(lsp_types::Uri, arcweft_tooling::model::TextEdit)> {
    if params.arguments.len() != 1 {
        return None;
    }
    let args: ToolingEditCommandArgs = serde_json::from_value(params.arguments[0].clone()).ok()?;
    Some((args.uri, args.edit))
}

#[derive(Debug, Deserialize)]
struct ToolingEditCommandArgs {
    uri: lsp_types::Uri,
    edit: arcweft_tooling::model::TextEdit,
}

fn extract<P: DeserializeOwned>(
    request: Request,
    method: &'static str,
) -> Result<(RequestId, P), SessionError> {
    let id = request.id;
    let params = decode(method, request.params)?;
    Ok((id, params))
}

impl WorkspaceEditPolicy {
    fn from_initialize(params: &InitializeParams) -> Self {
        Self {
            document_changes: params
                .capabilities
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.workspace_edit.as_ref())
                .and_then(|workspace_edit| workspace_edit.document_changes)
                .unwrap_or(false),
        }
    }

    fn normalize(
        self,
        mut edit: WorkspaceEdit,
        current_uri: &lsp_types::Uri,
        current_version: Option<i32>,
    ) -> WorkspaceEdit {
        if !self.document_changes || edit.document_changes.is_some() {
            return edit;
        }
        let Some(changes) = edit.changes.take() else {
            return edit;
        };
        edit.document_changes = Some(lsp_types::DocumentChanges::Edits(
            changes
                .into_iter()
                .map(|(uri, edits)| TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        version: (uri == *current_uri).then_some(current_version).flatten(),
                        uri,
                    },
                    edits: edits.into_iter().map(lsp_types::OneOf::Left).collect(),
                })
                .collect(),
        ));
        edit
    }
}

fn decode<P: DeserializeOwned>(method: &'static str, value: Value) -> Result<P, SessionError> {
    serde_json::from_value(value).map_err(|error| SessionError::InvalidParams { method, error })
}

#[cfg(test)]
mod tests;
