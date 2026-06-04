use crate::config::LspConfig;
use crate::session::{ArcweftLspSession, SessionError};
use lsp_server::{Connection, Message, ProtocolError};
use lsp_types::InitializeParams;
use thiserror::Error;

/// Runs the Arcweft language server over stdio.
pub fn run_stdio(config: LspConfig) -> Result<(), LspServerError> {
    let (connection, io_threads) = Connection::stdio();
    let result = run_connection(&connection, config);
    drop(connection);
    result?;
    io_threads.join()?;
    Ok(())
}

/// Runs the server over an already-created lsp-server connection.
pub fn run_connection(connection: &Connection, config: LspConfig) -> Result<(), LspServerError> {
    let mut session = ArcweftLspSession::new(config);
    let (initialize_id, initialize_params) = connection.initialize_start()?;
    let initialize_params: InitializeParams = serde_json::from_value(initialize_params)?;
    let capabilities = session.initialize(&initialize_params);
    connection.initialize_finish(
        initialize_id,
        serde_json::json!({
            "capabilities": capabilities,
        }),
    )?;

    let mut shutdown_requested = false;
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    shutdown_requested = true;
                    continue;
                }
                if shutdown_requested {
                    connection
                        .sender
                        .send(Message::Response(lsp_server::Response::new_err(
                            request.id,
                            lsp_server::ErrorCode::InvalidRequest as i32,
                            "request received after shutdown".to_owned(),
                        )))?;
                    continue;
                }
                let response = session.handle_request(request);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                if notification.method == "exit" {
                    return Ok(());
                }
                for publish in session.handle_notification(notification)? {
                    connection.sender.send(Message::Notification(publish))?;
                }
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
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
