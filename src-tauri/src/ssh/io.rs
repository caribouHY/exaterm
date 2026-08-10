use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use russh::keys::PublicKey;
use russh::{ChannelId, Disconnect};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, Mutex};
use tokio::time;

use crate::logger;
use crate::logger::LoggerState;
use crate::ssh::authentication_prompt::SshAuthenticationPromptState;
use crate::ssh::diagnostics::PendingHostKey;
use crate::ssh::host_key::{HostKeyVerifier, ProbeClientHandler};
use crate::terminal_control::TerminalControlState;
use crate::workspace::{emit_workspace_updated, WorkspaceState};

pub(super) struct SshSession {
    pub(super) handle: russh::client::Handle<SshClientHandler>,
    pub(super) channel: Arc<Mutex<russh::ChannelWriteHalf<russh::client::Msg>>>,
    pub(super) jump_handle: Option<russh::client::Handle<ProbeClientHandler>>,
}

pub(super) const SSH_READ_QUEUE_CAPACITY: usize = 1024;
pub(super) const SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS: usize = 1024;
const SSH_READ_DROP_STATUS_LINE: &[u8] =
    b"\r\n[ExaTerm] Some SSH output was dropped because the read queue was full.\r\n";
pub(super) const SSH_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const SSH_AUTH_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const SSH_CHANNEL_OPEN_TIMEOUT: Duration = Duration::from_secs(15);
pub(super) const SSH_PTY_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const SSH_SHELL_TIMEOUT: Duration = Duration::from_secs(10);
const SSH_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const SSH_RESIZE_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const SSH_CONNECT_TIMEOUT_ERROR: &str = "SSH connection timed out";
pub(super) const SSH_AUTH_TIMEOUT_ERROR: &str = "SSH authentication timed out";
pub(super) const SSH_CHANNEL_OPEN_TIMEOUT_ERROR: &str = "Opening the SSH channel timed out";
pub(super) const SSH_JUMP_CHANNEL_OPEN_TIMEOUT_ERROR: &str =
    "Opening the SSH jump channel timed out";
pub(super) const SSH_PTY_TIMEOUT_ERROR: &str = "PTY request timed out";
pub(super) const SSH_SHELL_TIMEOUT_ERROR: &str = "Shell request timed out";
pub(super) const SSH_WRITE_ERROR: &str = "SSH send error";
pub(super) const SSH_WRITE_TIMEOUT_ERROR: &str = "SSH send timed out";
const SSH_RESIZE_ERROR: &str = "SSH resize error";
const SSH_RESIZE_TIMEOUT_ERROR: &str = "SSH resize timed out";

pub(super) struct SshReadRequest {
    pub(super) data: Vec<u8>,
    pub(super) stream_kind: SshReadStreamKind,
}

#[derive(Clone, Copy)]
pub(super) enum SshReadStreamKind {
    Data,
    ExtendedData,
}

impl SshReadStreamKind {
    fn event_name(self, session_id: &str) -> String {
        match self {
            Self::Data => format!("ssh://data/{session_id}"),
            Self::ExtendedData => format!("ssh://error/{session_id}"),
        }
    }
}

#[derive(Default)]
pub(super) struct SshReadDropState {
    pub(super) dropped_chunks: usize,
    pub(super) dropped_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SshReadDropNotice {
    pub(super) dropped_chunks: usize,
    pub(super) dropped_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
struct SshReadOverflowEvent {
    dropped_chunks: usize,
    dropped_bytes: usize,
    queue_capacity: usize,
}

pub(super) fn ssh_read_overflow_event_name(session_id: &str) -> String {
    format!("ssh://read-overflow/{session_id}")
}

pub(super) fn record_ssh_read_drop(
    drop_state: &Arc<StdMutex<SshReadDropState>>,
    dropped_bytes: usize,
) -> Option<SshReadDropNotice> {
    let mut state = drop_state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.dropped_chunks = state.dropped_chunks.saturating_add(1);
    state.dropped_bytes = state.dropped_bytes.saturating_add(dropped_bytes);

    if state.dropped_chunks == 1 || state.dropped_chunks % SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS == 0
    {
        Some(SshReadDropNotice {
            dropped_chunks: state.dropped_chunks,
            dropped_bytes: state.dropped_bytes,
        })
    } else {
        None
    }
}

fn enqueue_ssh_read(
    app: &AppHandle,
    session_id: &str,
    read_tx: mpsc::Sender<SshReadRequest>,
    drop_state: Arc<StdMutex<SshReadDropState>>,
    request: SshReadRequest,
) {
    match read_tx.try_send(request) {
        Ok(()) | Err(TrySendError::Closed(_)) => {}
        Err(TrySendError::Full(request)) => {
            if let Some(notice) = record_ssh_read_drop(&drop_state, request.data.len()) {
                let _ = app.emit(
                    &ssh_read_overflow_event_name(session_id),
                    SshReadOverflowEvent {
                        dropped_chunks: notice.dropped_chunks,
                        dropped_bytes: notice.dropped_bytes,
                        queue_capacity: SSH_READ_QUEUE_CAPACITY,
                    },
                );
                let _ = app.emit(
                    &SshReadStreamKind::ExtendedData.event_name(session_id),
                    SSH_READ_DROP_STATUS_LINE.to_vec(),
                );
            }
        }
    }
}

pub(super) async fn run_ssh_operation_with_timeout<T, F>(
    timeout: Duration,
    timeout_message: &'static str,
    operation: F,
) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    match time::timeout(timeout, operation).await {
        Ok(result) => result,
        Err(_) => Err(timeout_message.to_string()),
    }
}

pub(super) async fn run_ssh_channel_operation_with_timeout<F>(
    timeout: Duration,
    timeout_message: &'static str,
    operation: F,
) -> Result<(), String>
where
    F: Future<Output = Result<(), String>>,
{
    run_ssh_operation_with_timeout(timeout, timeout_message, operation).await
}

#[derive(Clone)]
pub struct SshState {
    pub(super) sessions: Arc<Mutex<HashMap<String, Arc<Mutex<SshSession>>>>>,
    pub(super) pending_host_keys: Arc<Mutex<HashMap<String, PendingHostKey>>>,
    pub(crate) authentication_prompts: SshAuthenticationPromptState,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_host_keys: Arc::new(Mutex::new(HashMap::new())),
            authentication_prompts: SshAuthenticationPromptState::default(),
        }
    }
}

pub(super) struct SshClientHandler {
    pub(super) app: AppHandle,
    pub(super) session_id: String,
    pub(super) sessions: Arc<Mutex<HashMap<String, Arc<Mutex<SshSession>>>>>,
    pub(super) host_verifier: HostKeyVerifier,
    pub(super) terminals: TerminalControlState,
    pub(super) workspace: WorkspaceState,
    pub(super) logger: Option<LoggerState>,
    pub(super) read_tx: mpsc::Sender<SshReadRequest>,
    pub(super) read_drop_state: Arc<StdMutex<SshReadDropState>>,
}

impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        let result = self.host_verifier.check_key(server_public_key);
        async move { result }
    }

    fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let app = self.app.clone();
        let session_id = self.session_id.clone();
        let read_tx = self.read_tx.clone();
        let read_drop_state = self.read_drop_state.clone();
        let data = data.to_vec();
        async move {
            enqueue_ssh_read(
                &app,
                &session_id,
                read_tx,
                read_drop_state,
                SshReadRequest {
                    data,
                    stream_kind: SshReadStreamKind::Data,
                },
            );
            Ok(())
        }
    }

    fn extended_data(
        &mut self,
        _channel: ChannelId,
        _ext: u32,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let app = self.app.clone();
        let session_id = self.session_id.clone();
        let read_tx = self.read_tx.clone();
        let read_drop_state = self.read_drop_state.clone();
        let data = data.to_vec();
        async move {
            enqueue_ssh_read(
                &app,
                &session_id,
                read_tx,
                read_drop_state,
                SshReadRequest {
                    data,
                    stream_kind: SshReadStreamKind::ExtendedData,
                },
            );
            Ok(())
        }
    }

    fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut russh::client::Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let app = self.app.clone();
        let session_id = self.session_id.clone();
        let sessions = self.sessions.clone();
        let terminals = self.terminals.clone();
        let workspace = self.workspace.clone();
        let logger = self.logger.clone();
        async move {
            mark_disconnected_impl(
                &app,
                &session_id,
                &sessions,
                &terminals,
                &workspace,
                logger.as_ref(),
            )
            .await;
            Ok(())
        }
    }

    fn disconnected(
        &mut self,
        reason: russh::client::DisconnectReason<Self::Error>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let app = self.app.clone();
        let session_id = self.session_id.clone();
        let sessions = self.sessions.clone();
        let terminals = self.terminals.clone();
        let workspace = self.workspace.clone();
        let logger = self.logger.clone();
        async move {
            mark_disconnected_impl(
                &app,
                &session_id,
                &sessions,
                &terminals,
                &workspace,
                logger.as_ref(),
            )
            .await;
            match reason {
                russh::client::DisconnectReason::ReceivedDisconnect(_) => Ok(()),
                russh::client::DisconnectReason::Error(error) => Err(error),
            }
        }
    }
}

async fn mark_disconnected_impl(
    app: &AppHandle,
    session_id: &str,
    sessions: &Arc<Mutex<HashMap<String, Arc<Mutex<SshSession>>>>>,
    terminals: &TerminalControlState,
    workspace: &WorkspaceState,
    logger: Option<&LoggerState>,
) {
    let removed_session = sessions.lock().await.remove(session_id);
    if let Some(session) = removed_session {
        let session = session.lock().await;
        if let Some(jump_handle) = &session.jump_handle {
            let _ = jump_handle
                .disconnect(Disconnect::ByApplication, "Target disconnected", "en")
                .await;
        }
        terminals.mark_disconnected(session_id).await;
        if let Some(snapshot) = workspace.mark_disconnected(session_id).await {
            emit_workspace_updated(app, &snapshot);
        }
        let _ = app.emit("ssh://disconnected", session_id);
    }
    if let Some(logger_state) = logger {
        logger::clear_session_logs(logger_state, session_id).await;
    }
}

pub(super) fn spawn_ssh_read_processor(
    app: &AppHandle,
    session_id: &str,
    terminals: TerminalControlState,
    mut read_rx: mpsc::Receiver<SshReadRequest>,
) {
    let app = app.clone();
    let session_id = session_id.to_string();
    tokio::spawn(async move {
        while let Some(request) = read_rx.recv().await {
            terminals.append_output(&session_id, &request.data).await;
            let _ = app.emit(&request.stream_kind.event_name(&session_id), request.data);
        }
    });
}

pub async fn ssh_write(
    state: tauri::State<'_, SshState>,
    terminals: tauri::State<'_, crate::terminal_control::TerminalControlState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    write_data(&state, terminals.inner(), &session_id, data).await
}

pub async fn write_data(
    state: &SshState,
    terminals: &crate::terminal_control::TerminalControlState,
    session_id: &str,
    data: String,
) -> Result<(), String> {
    let session = {
        let sessions = state.sessions.lock().await;
        sessions.get(session_id).ok_or("Session not found")?.clone()
    };
    let data = terminals.encode_input(session_id, &data).await?;

    let channel = session.lock().await.channel.clone();
    run_ssh_channel_operation_with_timeout(
        SSH_WRITE_TIMEOUT,
        SSH_WRITE_TIMEOUT_ERROR,
        async move {
            channel
                .lock()
                .await
                .data_bytes(data)
                .await
                .map_err(|_| SSH_WRITE_ERROR.to_string())
        },
    )
    .await?;

    Ok(())
}

/// Resize SSH terminal
pub async fn ssh_resize(
    state: tauri::State<'_, SshState>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let session = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(&session_id)
            .ok_or("Session not found")?
            .clone()
    };

    let channel = session.lock().await.channel.clone();
    run_ssh_channel_operation_with_timeout(
        SSH_RESIZE_TIMEOUT,
        SSH_RESIZE_TIMEOUT_ERROR,
        async move {
            channel
                .lock()
                .await
                .window_change(cols, rows, 0, 0)
                .await
                .map_err(|_| SSH_RESIZE_ERROR.to_string())
        },
    )
    .await?;

    Ok(())
}

/// Disconnect SSH session
pub async fn ssh_disconnect(
    app: AppHandle,
    state: tauri::State<'_, SshState>,
    terminals: tauri::State<'_, TerminalControlState>,
    workspace: tauri::State<'_, WorkspaceState>,
    logger: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<(), String> {
    let session = state.sessions.lock().await.remove(&session_id);
    if let Some(session) = session {
        let session = session.lock().await;
        {
            let channel = session.channel.lock().await;
            let _ = channel.eof().await;
            let _ = channel.close().await;
        }
        let _ = session
            .handle
            .disconnect(Disconnect::ByApplication, "User disconnected", "en")
            .await;
        if let Some(jump_handle) = &session.jump_handle {
            let _ = jump_handle
                .disconnect(Disconnect::ByApplication, "User disconnected", "en")
                .await;
        }
    }
    terminals.mark_disconnected(&session_id).await;
    if let Some(snapshot) = workspace.mark_disconnected(&session_id).await {
        emit_workspace_updated(&app, &snapshot);
    }
    logger::clear_session_logs(&logger, &session_id).await;
    let _ = app.emit("ssh://disconnected", &session_id);
    Ok(())
}
