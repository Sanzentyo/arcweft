use crate::config::LspConfig;
use crate::custom::ArcweftCustomRequest;
use crate::diagnostics::{DocumentAnalysis, publish_diagnostics_from_analysis};
use crate::documents::{DocumentError, DocumentSnapshot, DocumentStore};
use crate::features;
use crate::positions::PositionEncoding;
use crate::profiles::{
    LspProfile, LspProfileResolver, file_path_from_uri,
    state::{AcceptedEnvironmentGeneration, AcceptedProfileKey, LspProfileState},
};
use crate::repl_command::{LspReplCommandExecutor, LspReplCommandRequest, LspReplCommandResponse};
use crate::uri_key::LspUriKey;
use arcweft_tooling::model::ToolingError;
use lsp_server::{ErrorCode, Notification, Request, RequestId, Response, ResponseError};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles,
    DidChangeWorkspaceFolders, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, Completion, DocumentSymbolRequest, GotoDefinition, HoverRequest,
    InlayHintRequest, PrepareRenameRequest, References, Rename, Request as LspRequest,
    WorkspaceSymbolRequest,
};
use lsp_types::{
    CodeActionOrCommand, CodeActionParams, CodeActionResponse, CompletionParams,
    CompletionResponse, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentSymbolParams,
    GotoDefinitionParams, HoverParams, HoverProviderCapability, InitializeParams, InlayHintParams,
    InlayHintServerCapabilities, OneOf, OptionalVersionedTextDocumentIdentifier, ReferenceParams,
    RenameOptions, RenameParams, ServerCapabilities, SignatureHelpOptions, TextDocumentEdit,
    TextDocumentSyncCapability, TextDocumentSyncKind, WorkDoneProgressOptions, WorkspaceEdit,
    WorkspaceSymbolParams,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

use self::overlay_authority::PendingSignatureAuthority;

/// Stateful Sans I/O session used by the stdio transport.
#[derive(Debug)]
pub struct ArcweftLspSession {
    documents: DocumentStore,
    default_profile: LspProfile,
    profiles_by_uri: BTreeMap<LspUriKey, LspProfile>,
    profile_keys_by_uri: BTreeMap<LspUriKey, AcceptedProfileKey>,
    analyses_by_uri: BTreeMap<LspUriKey, CachedDocumentAnalysis>,
    profile_resolver: LspProfileResolver,
    workspace_edit_policy: WorkspaceEditPolicy,
    position_encoding: PositionEncoding,
    signature_admission_open: bool,
    pending_signature_authority: PendingSignatureAuthority,
}

#[derive(Debug)]
struct CachedDocumentAnalysis {
    version: i32,
    profile_generation: Option<AcceptedEnvironmentGeneration>,
    analysis: Arc<DocumentAnalysis>,
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
    /// Source-edit planning found an invalid tooling range or overlap.
    #[error(transparent)]
    Tooling(#[from] ToolingError),
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
            profile_keys_by_uri: BTreeMap::new(),
            analyses_by_uri: BTreeMap::new(),
            profile_resolver,
            workspace_edit_policy: WorkspaceEditPolicy::default(),
            position_encoding: PositionEncoding::default(),
            signature_admission_open: true,
            pending_signature_authority: PendingSignatureAuthority::default(),
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
            document_symbol_provider: Some(OneOf::Left(true)),
            workspace_symbol_provider: Some(OneOf::Left(true)),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            })),
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
    #[cfg(test)]
    pub fn handle_request(&mut self, request: Request) -> Response {
        crate::requests::with_test_request_registry(|requests| {
            self.handle_request_with_repl_executor(request, None, requests)
        })
    }

    pub(crate) fn handle_request_with_requests(
        &mut self,
        request: Request,
        requests: &crate::requests::RequestRegistry,
    ) -> Response {
        self.handle_request_with_repl_executor(request, None, requests)
    }

    fn handle_request_with_repl_executor(
        &mut self,
        request: Request,
        repl: Option<&mut dyn LspReplCommandExecutor>,
        requests: &crate::requests::RequestRegistry,
    ) -> Response {
        let id = request.id.clone();
        self.try_handle_request(request, repl, requests)
            .unwrap_or_else(|error| {
                let code = match &error {
                    SessionError::Tooling(_) => ErrorCode::InternalError,
                    SessionError::InvalidParams { .. } | SessionError::Document(_) => {
                        ErrorCode::InvalidParams
                    }
                };
                Response::new_err(id, code as i32, error.to_string())
            })
    }

    /// Handles one notification with an isolated request registry in unit tests.
    #[cfg(test)]
    pub fn handle_notification(
        &mut self,
        notification: Notification,
    ) -> Result<Vec<Notification>, SessionError> {
        crate::requests::with_test_request_registry(|requests| {
            self.handle_notification_inner(notification, requests)
        })
    }

    pub(crate) fn handle_notification_with_requests(
        &mut self,
        notification: Notification,
        requests: &crate::requests::RequestRegistry,
    ) -> Result<Vec<Notification>, SessionError> {
        self.handle_notification_inner(notification, requests)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "notification dispatch keeps document mutation and lifecycle invalidation in protocol order"
    )]
    fn handle_notification_inner(
        &mut self,
        notification: Notification,
        requests: &crate::requests::RequestRegistry,
    ) -> Result<Vec<Notification>, SessionError> {
        if !self.signature_admission_open {
            return Ok(Vec::new());
        }
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params = decode::<DidOpenTextDocumentParams>(
                    DidOpenTextDocument::METHOD,
                    notification.params,
                )?;
                let uri = LspUriKey::from_uri(&params.text_document.uri);
                requests.cancel_uri(
                    &uri,
                    crate::requests::SignatureCancellationReason::DocumentChanged,
                );
                self.evict_signature_document_for_uri(&params.text_document.uri);
                let snapshot = self.documents.open(params, self.position_encoding);
                if self.attach_open_uri_to_accepted_profile(snapshot.uri()) {
                    self.mark_signature_authority_pending(snapshot.uri(), requests);
                    self.rebuild_profiles_affected_by_uri(snapshot.uri(), requests, true);
                } else {
                    self.refresh_profile_for_uri(snapshot.uri(), requests);
                }
                Ok(vec![self.refresh_document_diagnostics(&snapshot)])
            }
            DidChangeTextDocument::METHOD => {
                let params = decode::<DidChangeTextDocumentParams>(
                    DidChangeTextDocument::METHOD,
                    notification.params,
                )?;
                let uri = LspUriKey::from_uri(&params.text_document.uri);
                requests.cancel_uri(
                    &uri,
                    crate::requests::SignatureCancellationReason::DocumentChanged,
                );
                self.evict_signature_document_for_uri(&params.text_document.uri);
                let snapshot = self.documents.change(params, self.position_encoding)?;
                self.mark_signature_authority_pending(snapshot.uri(), requests);
                self.rebuild_profiles_affected_by_uri(snapshot.uri(), requests, true);
                Ok(vec![self.refresh_document_diagnostics(&snapshot)])
            }
            DidCloseTextDocument::METHOD => {
                let params = decode::<DidCloseTextDocumentParams>(
                    DidCloseTextDocument::METHOD,
                    notification.params,
                )?;
                let closed_key = LspUriKey::from_uri(&params.text_document.uri);
                requests.cancel_uri(
                    &closed_key,
                    crate::requests::SignatureCancellationReason::DocumentClosed,
                );
                self.evict_signature_document_for_uri(&params.text_document.uri);
                let affected_profiles = self
                    .profiles_by_uri
                    .iter()
                    .filter(|(key, profile)| {
                        *key != &closed_key
                            && profile.accepted_environment().is_some_and(|accepted| {
                                accepted
                                    .project()
                                    .sources()
                                    .by_uri(&params.text_document.uri)
                                    .is_some()
                            })
                    })
                    .fold(
                        Vec::<(LspUriKey, Arc<LspProfileState>)>::new(),
                        |mut affected, (key, profile)| {
                            if affected
                                .iter()
                                .all(|(_, state)| !Arc::ptr_eq(state, profile.state()))
                            {
                                affected.push((key.clone(), Arc::clone(profile.state())));
                            }
                            affected
                        },
                    );
                let removed_profile = self.profiles_by_uri.remove(&closed_key);
                self.profile_keys_by_uri.remove(&closed_key);
                self.pending_signature_authority
                    .remove_document(&closed_key);
                self.analyses_by_uri.remove(&closed_key);
                self.documents.close(&params.text_document.uri);
                if let Some(profile) = removed_profile {
                    let retained = self
                        .profiles_by_uri
                        .values()
                        .any(|current| Arc::ptr_eq(current.state(), profile.state()));
                    if !retained {
                        if let Some(accepted) = profile.accepted_environment() {
                            self.pending_signature_authority
                                .remove_profile(accepted.profile());
                        }
                        requests.cancel_profile_state(
                            profile.state(),
                            crate::requests::SignatureCancellationReason::ProfileClosing,
                        );
                        profile.state().shutdown();
                    }
                }
                for (key, _) in affected_profiles {
                    self.rebuild_profile_with_current_overlays(&key, requests, false);
                }
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
                self.invalidate_analysis_cache();
                self.refresh_profile_for_uri(&params.text_document.uri, requests);
                let Some(snapshot) = self.documents.get(&params.text_document.uri).cloned() else {
                    return Ok(Vec::new());
                };
                Ok(vec![self.refresh_document_diagnostics(&snapshot)])
            }
            DidChangeWatchedFiles::METHOD => {
                let _params = decode::<DidChangeWatchedFilesParams>(
                    DidChangeWatchedFiles::METHOD,
                    notification.params,
                )?;
                Ok(self.refresh_profile_for_open_documents(requests))
            }
            DidChangeConfiguration::METHOD => {
                let _params = decode::<DidChangeConfigurationParams>(
                    DidChangeConfiguration::METHOD,
                    notification.params,
                )?;
                Ok(self.refresh_profile_for_open_documents(requests))
            }
            DidChangeWorkspaceFolders::METHOD => {
                let params = decode::<DidChangeWorkspaceFoldersParams>(
                    DidChangeWorkspaceFolders::METHOD,
                    notification.params,
                )?;
                for removed in params.event.removed {
                    self.remove_workspace(&LspUriKey::from_uri(&removed.uri), requests);
                }
                Ok(self.refresh_profile_for_open_documents(requests))
            }
            _ => Ok(Vec::new()),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the LSP method dispatcher keeps protocol decoding and response ownership visible in one exhaustive match"
    )]
    fn try_handle_request(
        &mut self,
        request: Request,
        repl: Option<&mut dyn LspReplCommandExecutor>,
        requests: &crate::requests::RequestRegistry,
    ) -> Result<Response, SessionError> {
        match request.method.as_str() {
            Completion::METHOD => {
                let (id, params) = extract::<CompletionParams>(request, Completion::METHOD)?;
                Ok(self.completion_response(id, &params))
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
            GotoDefinition::METHOD => self.handle_definition_request(request, requests),
            PrepareRenameRequest::METHOD => {
                let (id, params) = extract::<lsp_types::TextDocumentPositionParams>(
                    request,
                    PrepareRenameRequest::METHOD,
                )?;
                let result = self
                    .document_for_params(&params.text_document.uri)
                    .and_then(|document| {
                        features::rename::prepare(
                            self.profile_for_uri(document.uri()),
                            document,
                            params.position,
                        )
                    });
                Ok(Response::new_ok(id, result))
            }
            Rename::METHOD => {
                let (id, params) = extract::<RenameParams>(request, Rename::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document_position.text_document.uri)
                    .and_then(|document| {
                        let edit = features::rename::rename(
                            self.profile_for_uri(document.uri()),
                            &self.documents,
                            document,
                            params.text_document_position.position,
                            &params.new_name,
                        )?;
                        Some(self.workspace_edit_policy.normalize(edit, &self.documents))
                    });
                Ok(Response::new_ok(id, result))
            }
            DocumentSymbolRequest::METHOD => {
                let (id, params) =
                    extract::<DocumentSymbolParams>(request, DocumentSymbolRequest::METHOD)?;
                let result = self
                    .document_for_params(&params.text_document.uri)
                    .map(|document| {
                        features::entry_roles::document_symbols(
                            self.profile_for_uri(document.uri()),
                            document,
                        )
                    });
                Ok(Response::new_ok(id, result))
            }
            WorkspaceSymbolRequest::METHOD => {
                let (id, params) =
                    extract::<WorkspaceSymbolParams>(request, WorkspaceSymbolRequest::METHOD)?;
                let result = features::entry_roles::workspace_symbols_for_profiles(
                    self.profiles_by_uri.values(),
                    &params.query,
                    self.position_encoding,
                );
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
            CodeActionRequest::METHOD => {
                let (id, params) = extract::<CodeActionParams>(request, CodeActionRequest::METHOD)?;
                let result = self.code_actions(&params)?;
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
            _ => Ok(Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported request `{}`", request.method),
            )),
        }
    }

    fn completion_response(&self, id: RequestId, params: &CompletionParams) -> Response {
        let uri = &params.text_document_position.text_document.uri;
        let items = features::completion::completions_at(
            self.profile_for_uri(uri),
            self.document_for_params(uri),
            params.text_document_position.position,
        );
        Response::new_ok(id, Some(CompletionResponse::Array(items)))
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

    fn handle_definition_request(
        &mut self,
        request: Request,
        requests: &crate::requests::RequestRegistry,
    ) -> Result<Response, SessionError> {
        let (id, params) = extract::<GotoDefinitionParams>(request, GotoDefinition::METHOD)?;
        let Some(document) = self
            .document_for_params(&params.text_document_position_params.text_document.uri)
            .cloned()
        else {
            return Ok(Response::new_ok(
                id,
                Option::<lsp_types::GotoDefinitionResponse>::None,
            ));
        };
        let profile = self.profile_for_uri(document.uri()).clone();
        let result = features::definition::definition(
            &profile,
            &params.text_document_position_params.text_document.uri,
            &self.documents,
            &document,
            params.text_document_position_params.position,
        );
        match result {
            Ok(result) => Ok(Response::new_ok(id, result)),
            Err(error) => {
                if error.schedules_profile_rebuild() {
                    self.rebuild_profiles_affected_by_uri(document.uri(), requests, false);
                }
                Ok(Response {
                    id,
                    result: None,
                    error: Some(ResponseError {
                        code: error.lsp_code(),
                        message: error.to_string(),
                        data: Some(serde_json::json!({ "code": error.stable_code() })),
                    }),
                })
            }
        }
    }

    fn document_for_params(&self, uri: &lsp_types::Uri) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    fn code_actions(&self, params: &CodeActionParams) -> Result<CodeActionResponse, SessionError> {
        let Some(document) = self.document_for_params(&params.text_document.uri) else {
            return Ok(Vec::new());
        };
        let analysis = self.cached_analysis(document).unwrap_or_else(|| {
            Arc::new(DocumentAnalysis::analyze_snapshot(
                document,
                self.profile_for_uri(document.uri()),
            ))
        });
        let actions = features::actions::actions(
            self.profile_for_uri(document.uri()),
            &params.text_document.uri,
            document,
            analysis.as_ref(),
            params.range.start,
        )?
        .into_iter()
        .map(|mut action| {
            action.edit = action
                .edit
                .map(|edit| self.workspace_edit_policy.normalize(edit, &self.documents));
            CodeActionOrCommand::CodeAction(action)
        })
        .collect();
        Ok(actions)
    }

    fn profile_for_uri(&self, uri: &lsp_types::Uri) -> &LspProfile {
        self.profiles_by_uri
            .get(&LspUriKey::from_uri(uri))
            .or_else(|| {
                self.profiles_by_uri.values().find(|profile| {
                    profile
                        .entry_selections()
                        .iter()
                        .any(|(_, selection)| selection.uri().as_ref() == Some(uri))
                })
            })
            .unwrap_or(&self.default_profile)
    }

    fn replace_analysis(&mut self, snapshot: &DocumentSnapshot) -> Arc<DocumentAnalysis> {
        let profile = self.profile_for_uri(snapshot.uri()).clone();
        let analysis = Arc::new(DocumentAnalysis::analyze_snapshot(snapshot, &profile));
        let profile_generation = profile
            .accepted_environment()
            .map(|environment| environment.generation());
        self.analyses_by_uri.insert(
            LspUriKey::from_uri(snapshot.uri()),
            CachedDocumentAnalysis {
                version: snapshot.version(),
                profile_generation,
                analysis: Arc::clone(&analysis),
            },
        );
        analysis
    }

    fn refresh_document_diagnostics(&mut self, snapshot: &DocumentSnapshot) -> Notification {
        if file_path_from_uri(snapshot.uri())
            .and_then(|path| {
                path.extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .map(str::to_owned)
            })
            .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
        {
            let analysis = self.replace_analysis(snapshot);
            self.publish_diagnostics_notification(snapshot, &analysis)
        } else {
            self.analyses_by_uri
                .remove(&LspUriKey::from_uri(snapshot.uri()));
            Notification::new(
                PublishDiagnostics::METHOD.to_owned(),
                lsp_types::PublishDiagnosticsParams::new(
                    snapshot.uri().clone(),
                    Vec::new(),
                    Some(snapshot.version()),
                ),
            )
        }
    }

    fn cached_analysis(&self, snapshot: &DocumentSnapshot) -> Option<Arc<DocumentAnalysis>> {
        let profile_generation = self
            .profile_for_uri(snapshot.uri())
            .accepted_environment()
            .map(|environment| environment.generation());
        self.analyses_by_uri
            .get(&LspUriKey::from_uri(snapshot.uri()))
            .filter(|cached| {
                cached.version == snapshot.version()
                    && cached.profile_generation == profile_generation
                    && Arc::ptr_eq(
                        cached.analysis.source_document(),
                        snapshot.source_document(),
                    )
            })
            .map(|cached| Arc::clone(&cached.analysis))
    }

    fn invalidate_analysis_cache(&mut self) {
        self.analyses_by_uri.clear();
    }

    pub(crate) fn begin_shutdown(&mut self, requests: &crate::requests::RequestRegistry) {
        if !self.signature_admission_open {
            return;
        }
        self.signature_admission_open = false;
        requests.close_admission();
        requests.cancel_all(crate::requests::SignatureCancellationReason::SessionShutdown);
        let mut states = Vec::<Arc<LspProfileState>>::new();
        for profile in self.profiles_by_uri.values() {
            let state = Arc::clone(profile.state());
            if states.iter().all(|current| !Arc::ptr_eq(current, &state)) {
                states.push(state);
            }
        }
        let default_state = Arc::clone(self.default_profile.state());
        if states
            .iter()
            .all(|current| !Arc::ptr_eq(current, &default_state))
        {
            states.push(default_state);
        }
        for state in states {
            state.shutdown();
        }
        self.documents.clear();
        self.profiles_by_uri.clear();
        self.profile_keys_by_uri.clear();
        self.pending_signature_authority.clear();
        self.analyses_by_uri.clear();
    }

    fn publish_diagnostics_notification(
        &self,
        snapshot: &DocumentSnapshot,
        analysis: &DocumentAnalysis,
    ) -> Notification {
        Notification::new(
            PublishDiagnostics::METHOD.to_owned(),
            publish_diagnostics_from_analysis(
                snapshot,
                self.profile_for_uri(snapshot.uri()),
                analysis,
            ),
        )
    }
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
        documents: &crate::documents::DocumentStore,
    ) -> WorkspaceEdit {
        if edit.document_changes.is_some() {
            return edit;
        }
        let Some(mut changes) = edit.changes.take() else {
            return edit;
        };
        for edits in changes.values_mut() {
            edits.sort_by(|left, right| {
                left.range
                    .start
                    .line
                    .cmp(&right.range.start.line)
                    .then_with(|| left.range.start.character.cmp(&right.range.start.character))
                    .then_with(|| left.range.end.line.cmp(&right.range.end.line))
                    .then_with(|| left.range.end.character.cmp(&right.range.end.character))
                    .then_with(|| left.new_text.cmp(&right.new_text))
            });
            edits.dedup();
        }
        if !self.document_changes {
            edit.changes = Some(changes);
            return edit;
        }
        let mut changes = changes.into_iter().collect::<Vec<_>>();
        changes.sort_by(|(left, _), (right, _)| left.as_str().cmp(right.as_str()));
        edit.document_changes = Some(lsp_types::DocumentChanges::Edits(
            changes
                .into_iter()
                .map(|(uri, edits)| TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        version: documents.get(&uri).map(DocumentSnapshot::version),
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
mod character_definition_tests;
mod lifecycle;
mod overlay_authority;
#[cfg(test)]
mod parser_diagnostic_tests;
mod profile_publication;
#[cfg(test)]
mod publication_gate_tests;
mod signature;
#[cfg(test)]
mod signature_cache_tests;
#[cfg(test)]
mod signature_stamp_tests;
#[cfg(test)]
mod tests;
