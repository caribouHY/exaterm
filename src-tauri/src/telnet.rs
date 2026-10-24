use serde::Deserialize;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::connect_attempt::{run_with_attempt, ConnectAttempt, ConnectAttemptState};
use crate::logger::LoggerState;
use crate::terminal_control::TerminalControlState;
use crate::workspace::{emit_workspace_updated, WorkspaceState};

mod codec;
mod session;
use codec::*;
use session::{SessionEvents, TelnetSession};

const TELNET_CONNECT_CANCELLED: &str = "The Telnet connection attempt was cancelled";
const TELNET_CONNECT_DUPLICATE: &str =
    "A Telnet connection attempt with this request ID already exists";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelnetConnectInput {
    pub host: String,
    pub port: u16,
    pub cols: u32,
    pub rows: u32,
    pub encoding: Option<String>,
    pub request_id: Option<String>,
}

pub(crate) struct TelnetConnectRuntime<'a> {
    pub app: &'a AppHandle,
    pub state: &'a TelnetState,
    pub terminals: &'a TerminalControlState,
    pub workspace: &'a WorkspaceState,
    pub logger: Option<&'a LoggerState>,
}

pub(crate) struct TelnetConnectRequest {
    pub host: String,
    pub port: u16,
    pub cols: u32,
    pub rows: u32,
    pub encoding: Option<String>,
}

#[derive(Clone)]
pub struct TelnetState {
    sessions: Arc<Mutex<HashMap<String, TelnetSession>>>,
    connect_attempts: ConnectAttemptState,
}

impl TelnetState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            connect_attempts: ConnectAttemptState::new(
                TELNET_CONNECT_CANCELLED,
                TELNET_CONNECT_DUPLICATE,
            ),
        }
    }
}

#[tauri::command]
pub async fn telnet_connect(
    app: AppHandle,
    state: tauri::State<'_, TelnetState>,
    terminals: tauri::State<'_, TerminalControlState>,
    workspace: tauri::State<'_, WorkspaceState>,
    logger: tauri::State<'_, LoggerState>,
    input: TelnetConnectInput,
) -> Result<String, crate::command_error::BackendCommandError> {
    let TelnetConnectInput {
        host,
        port,
        cols,
        rows,
        encoding,
        request_id,
    } = input;
    let request_id = request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::command_error::BackendCommandError::new(
                "telnet.connect_request_id_required",
                "A Telnet connection request ID is required",
            )
        })?
        .to_string();
    let attempt = state
        .connect_attempts
        .register(request_id)
        .map_err(crate::command_error::BackendCommandError::from)?;
    connect(
        TelnetConnectRuntime {
            app: &app,
            state: &state,
            terminals: &terminals,
            workspace: &workspace,
            logger: Some(&logger),
        },
        TelnetConnectRequest {
            host,
            port,
            cols,
            rows,
            encoding,
        },
        Some(attempt),
    )
    .await
    .map_err(Into::into)
}

pub(crate) async fn connect(
    runtime: TelnetConnectRuntime<'_>,
    request: TelnetConnectRequest,
    attempt: Option<ConnectAttempt>,
) -> Result<String, String> {
    let TelnetConnectRuntime {
        app,
        state,
        terminals,
        workspace,
        logger: logger_state,
    } = runtime;
    let TelnetConnectRequest {
        host,
        port,
        cols,
        rows,
        encoding,
    } = request;
    let session_id = Uuid::new_v4().to_string();
    let address = (host.clone(), port);
    session::connect(
        state,
        session::SessionOwner {
            terminals: terminals.clone(),
            workspace: workspace.clone(),
            logger: logger_state.cloned(),
            events: Arc::new(AppSessionEvents(app.clone())),
            session_id: session_id.clone(),
        },
        TelnetConnectRequest {
            host,
            port,
            cols,
            rows,
            encoding,
        },
        attempt,
        Box::pin(async move {
            TcpStream::connect(address)
                .await
                .map_err(|e| format!("Failed to connect over Telnet: {}", e))
        }),
    )
    .await?;
    Ok(session_id)
}

struct AppSessionEvents(AppHandle);

impl SessionEvents for AppSessionEvents {
    fn connected(&self, session_id: &str) {
        let _ = self.0.emit("telnet://connected", session_id);
    }

    fn data(&self, session_id: &str, output: crate::terminal_control::TerminalOutputEvent) {
        let _ = self
            .0
            .emit(&format!("telnet://data/{}", session_id), output);
    }

    fn error(&self, session_id: &str, error: &str) {
        let _ = self
            .0
            .emit(&format!("telnet://error/{}", session_id), error);
    }

    fn disconnected(
        &self,
        session_id: &str,
        snapshot: Option<crate::workspace::WorkspaceSnapshot>,
    ) {
        if let Some(snapshot) = snapshot {
            emit_workspace_updated(&self.0, &snapshot);
        }
        let _ = self.0.emit("telnet://disconnected", session_id);
    }
}

#[tauri::command]
pub async fn telnet_connect_cancel(
    state: tauri::State<'_, TelnetState>,
    request_id: String,
) -> Result<bool, crate::command_error::BackendCommandError> {
    Ok(state.connect_attempts.cancel(&request_id))
}

#[tauri::command]
pub async fn telnet_write(
    state: tauri::State<'_, TelnetState>,
    terminals: tauri::State<'_, TerminalControlState>,
    session_id: String,
    data: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    write_data(&state, terminals.inner(), &session_id, data)
        .await
        .map_err(Into::into)
}

pub async fn write_data(
    state: &TelnetState,
    terminals: &TerminalControlState,
    session_id: &str,
    data: String,
) -> Result<(), String> {
    let writer = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(session_id)
            .ok_or("Session not found")?
            .writer
            .clone()
    };

    writer
        .send(escape_user_data(
            terminals.encode_input(session_id, &data).await?,
        ))
        .await
        .map_err(|e| format!("Failed to send data: {}", e))
}

#[tauri::command]
pub async fn telnet_resize(
    state: tauri::State<'_, TelnetState>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), crate::command_error::BackendCommandError> {
    session::resize(&state, &session_id, cols, rows)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn telnet_disconnect(
    app: AppHandle,
    state: tauri::State<'_, TelnetState>,
    terminals: tauri::State<'_, TerminalControlState>,
    workspace: tauri::State<'_, WorkspaceState>,
    logger: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    disconnect(
        &app,
        &state,
        &terminals,
        &workspace,
        Some(&logger),
        &session_id,
    )
    .await
    .map_err(Into::into)
}

pub(crate) async fn disconnect(
    app: &AppHandle,
    state: &TelnetState,
    terminals: &TerminalControlState,
    workspace: &WorkspaceState,
    logger: Option<&LoggerState>,
    session_id: &str,
) -> Result<(), String> {
    let _ = (app, terminals, workspace);
    disconnect_session(state, logger, session_id).await;
    Ok(())
}

pub(crate) async fn disconnect_session(
    state: &TelnetState,
    logger: Option<&LoggerState>,
    session_id: &str,
) {
    session::disconnect(state, session_id).await;
    // GUI finalization can start logging after EOF cleanup. Explicit teardown
    // must also close that late log after waiting for the I/O owner.
    if let Some(logger) = logger {
        crate::logger::clear_session_logs(logger, session_id).await;
    }
}
