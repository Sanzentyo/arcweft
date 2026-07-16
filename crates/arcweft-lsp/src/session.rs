use crate::commands::ArcweftCommand;
use crate::config::LspConfig;
use crate::custom::ArcweftCustomRequest;
use crate::diagnostics::{DocumentAnalysis, publish_diagnostics_from_analysis};
use crate::documents::{DocumentError, DocumentSnapshot, DocumentStore, rebind_overlay};
use crate::features;
use crate::positions::PositionEncoding;
use crate::profiles::{
    LspProfile, LspProfileResolver, file_path_from_uri, register_profile_environment_with_overlays,
    state::{AcceptedEnvironmentGeneration, AcceptedProfileKey, LspProfileState},
};
use crate::repl_command::{LspReplCommandExecutor, LspReplCommandRequest, LspReplCommandResponse};
use crate::uri_key::LspUriKey;
use arcweft_source::SourceRevision;
use arcweft_tooling::model::ToolingError;
use arcweft_verify_lsp::workspace_edit_from_tooling_edit;
use lsp_server::{ErrorCode, Notification, Request, RequestId, Response, ResponseError};
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles,
    DidChangeWorkspaceFolders, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, Completion, ExecuteCommand, GotoDefinition, HoverRequest, InlayHintRequest,
    References, Request as LspRequest, SignatureHelpRequest,
};
use lsp_types::{
    CodeActionOrCommand, CodeActionParams, CodeActionResponse, CompletionParams,
    CompletionResponse, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, ExecuteCommandOptions,
    ExecuteCommandParams, GotoDefinitionParams, HoverParams, HoverProviderCapability,
    InitializeParams, InlayHintParams, InlayHintServerCapabilities, OneOf,
    OptionalVersionedTextDocumentIdentifier, ReferenceParams, ServerCapabilities,
    SignatureHelpOptions, SignatureHelpParams, TextDocumentEdit, TextDocumentSyncCapability,
    TextDocumentSyncKind, WorkDoneProgressOptions, WorkspaceEdit,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

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
}

#[derive(Debug)]
struct CachedDocumentAnalysis {
    version: i32,
    revision: SourceRevision,
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
        self.try_handle_request(request, repl)
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

    /// Handles one notification and returns notifications to publish.
    pub fn handle_notification(
        &mut self,
        notification: Notification,
    ) -> Result<Vec<Notification>, SessionError> {
        self.handle_notification_inner(notification, None)
    }

    pub(crate) fn handle_notification_with_requests(
        &mut self,
        notification: Notification,
        requests: &crate::requests::RequestRegistry,
    ) -> Result<Vec<Notification>, SessionError> {
        self.handle_notification_inner(notification, Some(requests))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "notification dispatch keeps document mutation and lifecycle invalidation in protocol order"
    )]
    fn handle_notification_inner(
        &mut self,
        notification: Notification,
        requests: Option<&crate::requests::RequestRegistry>,
    ) -> Result<Vec<Notification>, SessionError> {
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params = decode::<DidOpenTextDocumentParams>(
                    DidOpenTextDocument::METHOD,
                    notification.params,
                )?;
                let uri = LspUriKey::from_uri(&params.text_document.uri);
                if let Some(requests) = requests {
                    requests.cancel_uri(
                        &uri,
                        crate::requests::SignatureCancellationReason::DocumentChanged,
                    );
                }
                let snapshot = self.documents.open(params, self.position_encoding);
                self.refresh_profile_for_uri(snapshot.uri(), requests);
                self.rebuild_profiles_affected_by_uri(snapshot.uri(), requests, true);
                Ok(vec![self.refresh_document_diagnostics(&snapshot)])
            }
            DidChangeTextDocument::METHOD => {
                let params = decode::<DidChangeTextDocumentParams>(
                    DidChangeTextDocument::METHOD,
                    notification.params,
                )?;
                let uri = LspUriKey::from_uri(&params.text_document.uri);
                if let Some(requests) = requests {
                    requests.cancel_uri(
                        &uri,
                        crate::requests::SignatureCancellationReason::DocumentChanged,
                    );
                }
                let snapshot = self.documents.change(params, self.position_encoding)?;
                self.rebuild_profiles_affected_by_uri(snapshot.uri(), requests, true);
                Ok(vec![self.refresh_document_diagnostics(&snapshot)])
            }
            DidCloseTextDocument::METHOD => {
                let params = decode::<DidCloseTextDocumentParams>(
                    DidCloseTextDocument::METHOD,
                    notification.params,
                )?;
                let closed_key = LspUriKey::from_uri(&params.text_document.uri);
                if let Some(requests) = requests {
                    requests.cancel_uri(
                        &closed_key,
                        crate::requests::SignatureCancellationReason::DocumentClosed,
                    );
                }
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
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>();
                let removed_profile = self.profiles_by_uri.remove(&closed_key);
                self.profile_keys_by_uri.remove(&closed_key);
                self.analyses_by_uri.remove(&closed_key);
                self.documents.close(&params.text_document.uri);
                if let Some(profile) = removed_profile {
                    let retained = self
                        .profiles_by_uri
                        .values()
                        .any(|current| Arc::ptr_eq(current.state(), profile.state()));
                    if !retained {
                        if let Some(requests) = requests {
                            requests.cancel_profile_state(
                                profile.state(),
                                crate::requests::SignatureCancellationReason::ProfileClosing,
                            );
                        }
                        profile.state().shutdown();
                    }
                }
                for key in affected_profiles {
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
                self.rebuild_profiles_affected_by_uri(&params.text_document.uri, requests, false);
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
                if let Some(requests) = requests {
                    for removed in params.event.removed {
                        self.remove_workspace(&LspUriKey::from_uri(&removed.uri), requests);
                    }
                }
                Ok(self.refresh_profile_for_open_documents(requests))
            }
            _ => Ok(Vec::new()),
        }
    }

    fn try_handle_request(
        &mut self,
        request: Request,
        repl: Option<&mut dyn LspReplCommandExecutor>,
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
            GotoDefinition::METHOD => self.handle_definition_request(request),
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

    fn handle_definition_request(&mut self, request: Request) -> Result<Response, SessionError> {
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
                    self.rebuild_profiles_affected_by_uri(document.uri(), None, false);
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
            Arc::new(DocumentAnalysis::analyze(
                document.text(),
                document.line_index().position_encoding(),
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
            action.edit = action.edit.map(|edit| {
                self.workspace_edit_policy
                    .normalize(edit, document.uri(), document.version())
            });
            CodeActionOrCommand::CodeAction(action)
        })
        .collect();
        Ok(actions)
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
        let Ok(edit) = workspace_edit_from_tooling_edit(
            &uri,
            &edit,
            document.source_document(),
            document.line_index(),
        ) else {
            return Value::Null;
        };
        let edit = self
            .workspace_edit_policy
            .normalize(edit, document.uri(), document.version());
        serde_json::to_value(edit).unwrap_or(Value::Null)
    }

    fn refresh_profile_for_uri(
        &mut self,
        uri: &lsp_types::Uri,
        requests: Option<&crate::requests::RequestRegistry>,
    ) {
        let key = LspUriKey::from_uri(uri);
        let previous_profile = self.profiles_by_uri.get(&key).cloned();
        let previous_accepted = previous_profile
            .as_ref()
            .and_then(LspProfile::accepted_environment);
        let state = self.profiles_by_uri.get(&key).map_or_else(
            || Arc::new(LspProfileState::new()),
            |profile| Arc::clone(profile.state()),
        );
        let profile = self.profile_resolver.resolve_for_uri_with_state(uri, state);
        if let (Some(requests), Some(previous)) = (requests, previous_profile.as_ref())
            && !Arc::ptr_eq(previous.state(), profile.state())
        {
            requests.cancel_profile_state(
                previous.state(),
                crate::requests::SignatureCancellationReason::ProfileRemapped,
            );
        }
        let current_accepted = profile.accepted_environment();
        if let (Some(requests), Some(previous)) = (requests, previous_accepted.as_ref())
            && current_accepted
                .as_ref()
                .is_none_or(|current| !Arc::ptr_eq(previous, current))
        {
            requests.cancel_accepted(
                previous,
                crate::requests::SignatureCancellationReason::AcceptedReplaced,
            );
            previous.clear_caches();
        }
        if let Some(accepted) = current_accepted {
            self.profile_keys_by_uri
                .insert(key.clone(), accepted.profile().clone());
        } else {
            self.profile_keys_by_uri.remove(&key);
        }
        self.profiles_by_uri.insert(key, profile);
    }

    fn refresh_profile_for_open_documents(
        &mut self,
        requests: Option<&crate::requests::RequestRegistry>,
    ) -> Vec<Notification> {
        self.invalidate_analysis_cache();
        let snapshots = self.documents.snapshots().cloned().collect::<Vec<_>>();
        for snapshot in &snapshots {
            self.refresh_profile_for_uri(snapshot.uri(), requests);
        }
        for snapshot in &snapshots {
            self.rebuild_profiles_affected_by_uri(snapshot.uri(), requests, false);
        }
        snapshots
            .iter()
            .map(|snapshot| self.refresh_document_diagnostics(snapshot))
            .collect()
    }

    fn profile_for_uri(&self, uri: &lsp_types::Uri) -> &LspProfile {
        self.profiles_by_uri
            .get(&LspUriKey::from_uri(uri))
            .unwrap_or(&self.default_profile)
    }

    fn rebuild_profiles_affected_by_uri(
        &mut self,
        changed: &lsp_types::Uri,
        requests: Option<&crate::requests::RequestRegistry>,
        allow_unchanged_project: bool,
    ) {
        let keys = self
            .profiles_by_uri
            .iter()
            .filter(|(_, profile)| {
                profile
                    .accepted_environment()
                    .is_some_and(|accepted| accepted.project().sources().by_uri(changed).is_some())
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in keys {
            self.rebuild_profile_with_current_overlays(&key, requests, allow_unchanged_project);
        }
    }

    fn rebuild_profile_with_current_overlays(
        &mut self,
        key: &LspUriKey,
        requests: Option<&crate::requests::RequestRegistry>,
        allow_unchanged_project: bool,
    ) {
        let Some(profile) = self.profiles_by_uri.get(key).cloned() else {
            return;
        };
        let Some(resolved) = profile.resolved_profile() else {
            return;
        };
        let Some(previous) = profile.accepted_environment() else {
            return;
        };
        let manifest_uri = previous.profile().manifest_key().to_uri();
        let Some(manifest_path) = file_path_from_uri(&manifest_uri) else {
            return;
        };
        let mut overlay_seeds = Vec::new();
        let mut overlay_entries = Vec::new();
        for snapshot in self.documents.snapshots() {
            let Some(accepted_source) = previous.project().sources().by_uri(snapshot.uri()) else {
                continue;
            };
            let version = snapshot.version();
            let Ok(document) = rebind_overlay(snapshot, accepted_source) else {
                return;
            };
            let Some(path) = file_path_from_uri(snapshot.uri()) else {
                return;
            };
            let Ok(seed) = arcweft_project_loader::topology::ProfileTopologyOverlaySeed::try_new(
                path,
                snapshot.text().to_owned(),
            ) else {
                return;
            };
            overlay_entries.push((
                crate::uri_key::LspUriKey::from_uri(snapshot.uri()),
                crate::profiles::state::AcceptedOverlayEntry::new(
                    version,
                    document.identity().clone(),
                ),
            ));
            overlay_seeds.push(seed);
        }
        let Ok(overlays) = crate::profiles::state::AcceptedOverlaySet::try_new(overlay_entries)
        else {
            return;
        };
        if allow_unchanged_project
            && let Ok(candidate) =
                crate::profiles::state::AcceptedProfileCandidate::try_from_unchanged_project(
                    &previous,
                    overlays.clone(),
                )
        {
            let _ = self.replace_profile_candidate(key, &profile, &previous, candidate, requests);
            return;
        }
        let registered = register_profile_environment_with_overlays(
            &manifest_path,
            resolved.id(),
            &overlay_seeds,
            overlays.clone(),
            Some(previous.world().environment()),
        );
        let Ok(registered) = registered else {
            let _ = self.record_failed_replacement(profile.state(), &previous);
            return;
        };
        let (candidate, characters, topology) = registered.into_parts();
        if self
            .replace_profile_candidate(key, &profile, &previous, candidate, requests)
            .is_err()
        {
            return;
        }
        if let Some(profile) = self.profiles_by_uri.get_mut(key) {
            crate::profiles::apply_registered_topology(profile, &topology, characters);
        }
    }

    fn replace_profile_candidate(
        &mut self,
        key: &LspUriKey,
        profile: &LspProfile,
        expected: &Arc<crate::profiles::state::AcceptedProfileEnvironment>,
        candidate: crate::profiles::state::AcceptedProfileCandidate,
        requests: Option<&crate::requests::RequestRegistry>,
    ) -> Result<
        Arc<crate::profiles::state::AcceptedProfileEnvironment>,
        crate::session::lifecycle::AcceptedPublicationError,
    > {
        let accepted =
            self.publish_accepted_candidate(profile.state(), expected, candidate, requests)?;
        self.profile_keys_by_uri
            .insert(key.clone(), accepted.profile().clone());
        Ok(accepted)
    }

    fn replace_analysis(&mut self, snapshot: &DocumentSnapshot) -> Arc<DocumentAnalysis> {
        let profile = self.profile_for_uri(snapshot.uri()).clone();
        let analysis = Arc::new(DocumentAnalysis::analyze_project(
            snapshot.text(),
            snapshot.line_index().position_encoding(),
            &profile,
            snapshot.uri(),
        ));
        let profile_generation = profile
            .accepted_environment()
            .map(|environment| environment.generation());
        self.analyses_by_uri.insert(
            LspUriKey::from_uri(snapshot.uri()),
            CachedDocumentAnalysis {
                version: snapshot.version(),
                revision: analysis.source_revision(),
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
        let revision = SourceRevision::for_utf8(snapshot.text());
        let profile_generation = self
            .profile_for_uri(snapshot.uri())
            .accepted_environment()
            .map(|environment| environment.generation());
        self.analyses_by_uri
            .get(&LspUriKey::from_uri(snapshot.uri()))
            .filter(|cached| {
                cached.version == snapshot.version()
                    && cached.revision == revision
                    && cached.profile_generation == profile_generation
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
        current_version: i32,
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
                        version: (uri == *current_uri).then_some(current_version),
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
mod signature;
#[cfg(test)]
mod tests;
