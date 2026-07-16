use crate::config::LspConfig;
use crate::requests::{
    SignatureCancellationReason, SignatureRequestRuntime, signature::SignatureAcquireError,
};
use crate::session::{ArcweftLspSession, SessionError};
use lsp_server::{Connection, Message, ProtocolError};
use lsp_types::request::{Request as LspRequest, SignatureHelpRequest};
use lsp_types::{InitializeParams, SignatureHelp, SignatureHelpParams};
use std::sync::{Arc, PoisonError, RwLock};
use thiserror::Error;

/// Runs the Arcweft language server over stdio.
pub fn run_stdio(config: &LspConfig) -> Result<(), LspServerError> {
    let (connection, io_threads) = Connection::stdio();
    let result = run_connection(&connection, config);
    drop(connection);
    result?;
    io_threads.join()?;
    Ok(())
}

/// Runs the server over an already-created lsp-server connection.
#[allow(
    clippy::too_many_lines,
    reason = "the transport loop preserves one ordered shutdown, cancellation, request, and notification state machine"
)]
pub fn run_connection(connection: &Connection, config: &LspConfig) -> Result<(), LspServerError> {
    let session = Arc::new(RwLock::new(ArcweftLspSession::new(config)));
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let initialize_params: InitializeParams = serde_json::from_value(initialize_params)?;
    let capabilities = session
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .initialize(&initialize_params);
    connection.initialize_finish(
        initialize_id,
        serde_json::json!({
            "capabilities": capabilities,
        }),
    )?;

    let runtime = SignatureRequestRuntime::new(connection, Arc::clone(&session))?;
    let result = (|| {
        let mut shutdown_requested = false;
        for message in &connection.receiver {
            match message {
                Message::Request(request) => {
                    if connection.handle_shutdown(&request)? {
                        shutdown_requested = true;
                        session
                            .write()
                            .unwrap_or_else(PoisonError::into_inner)
                            .begin_shutdown(runtime.registry());
                        continue;
                    }
                    if shutdown_requested {
                        connection.sender.send(Message::Response(
                            lsp_server::Response::new_err(
                                request.id,
                                lsp_server::ErrorCode::InvalidRequest as i32,
                                "request received after shutdown".to_owned(),
                            ),
                        ))?;
                        continue;
                    }
                    if request.method == SignatureHelpRequest::METHOD {
                        let id = request.id.clone();
                        let params =
                            match serde_json::from_value::<SignatureHelpParams>(request.params) {
                                Ok(params) => params,
                                Err(error) => {
                                    connection.sender.send(Message::Response(
                                        lsp_server::Response::new_err(
                                            id,
                                            lsp_server::ErrorCode::InvalidParams as i32,
                                            error.to_string(),
                                        ),
                                    ))?;
                                    continue;
                                }
                            };
                        let prepared = session
                            .read()
                            .unwrap_or_else(PoisonError::into_inner)
                            .prepare_signature_request(id.clone(), params, runtime.registry());
                        match prepared {
                            Ok(prepared) => {
                                if let Err(error) = runtime.submit(prepared) {
                                    let error = SignatureAcquireError::from(error);
                                    connection.sender.send(Message::Response(
                                        lsp_server::Response::new_err(
                                            id,
                                            error.lsp_code().unwrap_or(
                                                lsp_server::ErrorCode::RequestFailed as i32,
                                            ),
                                            error.to_string(),
                                        ),
                                    ))?;
                                }
                            }
                            Err(error) => {
                                let response = error.lsp_code().map_or_else(
                                    || {
                                        lsp_server::Response::new_ok(
                                            id.clone(),
                                            Option::<SignatureHelp>::None,
                                        )
                                    },
                                    |code| {
                                        lsp_server::Response::new_err(
                                            id.clone(),
                                            code,
                                            error.to_string(),
                                        )
                                    },
                                );
                                connection.sender.send(Message::Response(response))?;
                            }
                        }
                        continue;
                    }
                    let response = session
                        .write()
                        .unwrap_or_else(PoisonError::into_inner)
                        .handle_request(request);
                    connection.sender.send(Message::Response(response))?;
                }
                Message::Notification(notification) => {
                    if notification.method == "exit" {
                        break;
                    }
                    if notification.method == "$/cancelRequest" {
                        if let Some(id) = notification
                            .params
                            .get("id")
                            .and_then(|value| serde_json::from_value(value.clone()).ok())
                        {
                            runtime
                                .registry()
                                .cancel(&id, SignatureCancellationReason::ClientCancelled);
                        }
                        continue;
                    }
                    for publish in session
                        .write()
                        .unwrap_or_else(PoisonError::into_inner)
                        .handle_notification_with_requests(notification, runtime.registry())?
                    {
                        connection.sender.send(Message::Notification(publish))?;
                    }
                }
                Message::Response(_) => {}
            }
        }
        Ok(())
    })();
    session
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .begin_shutdown(runtime.registry());
    runtime.shutdown();
    result
}

/// Server transport error.
#[derive(Debug, Error)]
pub enum LspServerError {
    /// LSP protocol handshake failed.
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    /// JSON parameters failed to decode.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Session request handling failed.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// Signature request runtime initialization failed.
    #[error("signature request runtime initialization failed: {0}")]
    RequestRuntime(String),
    /// Message channel was disconnected.
    #[error("LSP message channel disconnected")]
    ChannelSend,
    /// Stdio thread failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<crossbeam_channel::SendError<Message>> for LspServerError {
    fn from(_: crossbeam_channel::SendError<Message>) -> Self {
        Self::ChannelSend
    }
}

impl From<crate::requests::RequestRuntimeError> for LspServerError {
    fn from(error: crate::requests::RequestRuntimeError) -> Self {
        Self::RequestRuntime(error.to_string())
    }
}
