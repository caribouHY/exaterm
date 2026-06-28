use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use russh::keys::decode_secret_key;
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::{Algorithm, PrivateKey, PublicKey};
use russh::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::config::{config_load, AppConfig, SavedConnection, SshConfig};
use crate::ssh_known_hosts::{
    endpoint_cache_key, inspect_host_key_with_path, known_hosts_path, write_trusted_host,
    HostKeyCheckResult, HostKeyCheckStatus,
};
use crate::terminal_control::{TerminalControlState, TerminalProtocol};
use crate::workspace::{emit_workspace_updated, WorkspaceState};
use crate::{logger, logger::LoggerState};

/// SSH session state shared across async tasks
struct SshSession {
    handle: russh::client::Handle<SshClientHandler>,
    channel: Arc<Mutex<russh::ChannelWriteHalf<russh::client::Msg>>>,
    jump_handle: Option<russh::client::Handle<ProbeClientHandler>>,
}

const SSH_READ_QUEUE_CAPACITY: usize = 1024;
const SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS: usize = 1024;
const SSH_READ_DROP_STATUS_LINE: &[u8] =
    b"\r\n[ExaTerm] Some SSH output was dropped because the read queue was full.\r\n";

struct SshReadRequest {
    data: Vec<u8>,
    stream_kind: SshReadStreamKind,
}

#[derive(Clone, Copy)]
enum SshReadStreamKind {
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
struct SshReadDropState {
    dropped_chunks: usize,
    dropped_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct SshReadDropNotice {
    dropped_chunks: usize,
    dropped_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
struct SshReadOverflowEvent {
    dropped_chunks: usize,
    dropped_bytes: usize,
    queue_capacity: usize,
}

fn ssh_read_overflow_event_name(session_id: &str) -> String {
    format!("ssh://read-overflow/{session_id}")
}

fn record_ssh_read_drop(
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

#[derive(Clone)]
struct PendingHostKey {
    key: PublicKey,
}

#[derive(Clone)]
struct SshDiagnostic {
    app: AppHandle,
    request_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct SshDiagnosticEvent {
    level: &'static str,
    message: String,
}

fn ssh_diagnostic_event_name(request_id: &str) -> String {
    format!("ssh://connect-diagnostic/{request_id}")
}

fn emit_ssh_diagnostic(
    app: &AppHandle,
    request_id: Option<&str>,
    level: &'static str,
    message: impl Into<String>,
) {
    let Some(request_id) = request_id else {
        return;
    };

    let _ = app.emit(
        &ssh_diagnostic_event_name(request_id),
        SshDiagnosticEvent {
            level,
            message: message.into(),
        },
    );
}

impl SshDiagnostic {
    fn new(app: &AppHandle, request_id: Option<String>) -> Self {
        Self {
            app: app.clone(),
            request_id,
        }
    }

    fn info(&self, message: impl Into<String>) {
        emit_ssh_diagnostic(&self.app, self.request_id.as_deref(), "info", message);
    }

    fn error(&self, message: impl Into<String>) {
        emit_ssh_diagnostic(&self.app, self.request_id.as_deref(), "error", message);
    }
}

/// Global SSH session store
#[derive(Clone)]
pub struct SshState {
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<SshSession>>>>>,
    pending_host_keys: Arc<Mutex<HashMap<String, PendingHostKey>>>,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_host_keys: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Clone, Copy)]
enum HostKeyVerificationMode {
    Probe,
    Enforce,
}

#[derive(Clone)]
struct HostKeyVerifier {
    host: String,
    port: u16,
    known_hosts_path: PathBuf,
    mode: HostKeyVerificationMode,
    observed_key: Arc<StdMutex<Option<PendingHostKey>>>,
    last_result: Arc<StdMutex<Option<HostKeyCheckResult>>>,
}

impl HostKeyVerifier {
    fn probe(host: String, port: u16) -> Self {
        Self::with_path(
            host,
            port,
            HostKeyVerificationMode::Probe,
            known_hosts_path(),
        )
    }

    fn enforce(host: String, port: u16) -> Self {
        Self::with_path(
            host,
            port,
            HostKeyVerificationMode::Enforce,
            known_hosts_path(),
        )
    }

    fn with_path(
        host: String,
        port: u16,
        mode: HostKeyVerificationMode,
        known_hosts_path: PathBuf,
    ) -> Self {
        Self {
            host,
            port,
            known_hosts_path,
            mode,
            observed_key: Arc::new(StdMutex::new(None)),
            last_result: Arc::new(StdMutex::new(None)),
        }
    }

    fn check_key(&self, server_public_key: &PublicKey) -> Result<bool, russh::Error> {
        {
            let mut observed = self
                .observed_key
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *observed = Some(PendingHostKey {
                key: server_public_key.clone(),
            });
        }

        let result = inspect_host_key_with_path(
            &self.host,
            self.port,
            server_public_key,
            &self.known_hosts_path,
        )
        .map_err(|error| russh::Error::IO(std::io::Error::new(std::io::ErrorKind::Other, error)))?;

        {
            let mut decision = self
                .last_result
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *decision = Some(result.clone());
        }

        Ok(match self.mode {
            HostKeyVerificationMode::Probe => true,
            HostKeyVerificationMode::Enforce => result.status == HostKeyCheckStatus::Trusted,
        })
    }

    fn last_result(&self) -> Option<HostKeyCheckResult> {
        self.last_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn observed_key(&self) -> Option<PendingHostKey> {
        self.observed_key
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

/// russh client handler — receives data from server
struct SshClientHandler {
    app: AppHandle,
    session_id: String,
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<SshSession>>>>>,
    host_verifier: HostKeyVerifier,
    terminals: TerminalControlState,
    workspace: WorkspaceState,
    logger: Option<LoggerState>,
    read_tx: mpsc::Sender<SshReadRequest>,
    read_drop_state: Arc<StdMutex<SshReadDropState>>,
}

struct ProbeClientHandler {
    host_verifier: HostKeyVerifier,
}

impl russh::client::Handler for ProbeClientHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        let result = self.host_verifier.check_key(server_public_key);
        async move { result }
    }
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

fn spawn_ssh_read_processor(
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConnectResult {
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct SshConnectOptions {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub auth_method: Option<String>,
    pub private_key_path: Option<String>,
    pub key_passphrase: Option<String>,
    pub jump_profile_id: Option<String>,
    pub jump_password: Option<String>,
    pub jump_key_passphrase: Option<String>,
    pub cols: u32,
    pub rows: u32,
    pub encoding: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshJumpProfile {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub private_key_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SshAuthRequest {
    Password {
        password: String,
    },
    PublicKey {
        private_key_path: String,
        key_passphrase: Option<String>,
    },
}

fn build_auth_request(
    auth_method: Option<String>,
    password: String,
    private_key_path: Option<String>,
    key_passphrase: Option<String>,
) -> Result<SshAuthRequest, String> {
    let method = auth_method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("password");

    match method {
        "password" => Ok(SshAuthRequest::Password { password }),
        "public_key" => {
            let private_key_path = private_key_path.unwrap_or_default().trim().to_string();
            if private_key_path.is_empty() {
                return Err("SSH公開鍵認証エラー: 秘密鍵ファイルを指定してください".to_string());
            }

            let key_passphrase = key_passphrase
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            Ok(SshAuthRequest::PublicKey {
                private_key_path,
                key_passphrase,
            })
        }
        _ => Err("SSH認証方式が不正です".to_string()),
    }
}

fn normalize_profile_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_profile_auth_method(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("password") => Ok("password".into()),
        Some("public_key") => Ok("public_key".into()),
        Some(_) => Err("SSH認証方式が不正です".into()),
    }
}

fn normalize_connection_type(profile: &SavedConnection) -> String {
    profile.connection_type.trim().to_ascii_lowercase()
}

pub fn resolve_jump_profile(
    config: &AppConfig,
    jump_profile_id: Option<&str>,
    target_profile_id: Option<&str>,
) -> Result<Option<SshJumpProfile>, String> {
    let Some(jump_profile_id) =
        jump_profile_id.and_then(|value| normalize_profile_string(Some(value)))
    else {
        return Ok(None);
    };

    if target_profile_id
        .and_then(|value| normalize_profile_string(Some(value)))
        .as_deref()
        == Some(jump_profile_id.as_str())
    {
        return Err("SSH踏み台プロファイルに自分自身は指定できません".into());
    }

    let profile = config
        .saved_connections
        .iter()
        .find(|profile| profile.id == jump_profile_id)
        .ok_or_else(|| "SSH踏み台プロファイルが見つかりません".to_string())?;

    if normalize_connection_type(profile) != "ssh" {
        return Err("SSH踏み台にはSSHプロファイルのみ指定できます".into());
    }

    if normalize_profile_string(profile.jump_profile_id.as_deref()).is_some() {
        return Err("SSH踏み台の多段指定には対応していません".into());
    }

    let host = normalize_profile_string(profile.host.as_deref())
        .ok_or_else(|| "SSH踏み台プロファイルにホストが設定されていません".to_string())?;
    let username = normalize_profile_string(profile.username.as_deref())
        .ok_or_else(|| "SSH踏み台プロファイルにユーザー名が設定されていません".to_string())?;
    let auth_method = normalize_profile_auth_method(profile.auth_method.as_deref())?;
    let private_key_path = normalize_profile_string(profile.private_key_path.as_deref());
    if auth_method == "public_key" && private_key_path.is_none() {
        return Err("SSH踏み台プロファイルに秘密鍵ファイルが設定されていません".to_string());
    }

    Ok(Some(SshJumpProfile {
        id: profile.id.clone(),
        host,
        port: profile.port.unwrap_or(22),
        username,
        auth_method,
        private_key_path,
    }))
}

fn expand_percent_env_vars(path: &str) -> String {
    let mut expanded = String::new();
    let mut rest = path;

    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        if let Some(end) = after_start.find('%') {
            let name = &after_start[..end];
            if let Ok(value) = std::env::var(name) {
                expanded.push_str(&value);
            } else {
                expanded.push('%');
                expanded.push_str(name);
                expanded.push('%');
            }
            rest = &after_start[end + 1..];
        } else {
            expanded.push_str(&rest[start..]);
            return expanded;
        }
    }

    expanded.push_str(rest);
    expanded
}

fn normalize_auth_path(path: &str) -> PathBuf {
    let trimmed = path.trim().trim_matches('"').trim_matches('\'');
    let expanded = expand_percent_env_vars(trimmed);

    if let Some(rest) = expanded
        .strip_prefix("~/")
        .or_else(|| expanded.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(expanded)
}

fn private_key_format_hint(secret: &str) -> Result<(), String> {
    let first_line = secret.lines().find(|line| !line.trim().is_empty());

    match first_line.map(str::trim) {
        Some(line) if line.starts_with("ssh-") => Err(
            "秘密鍵ファイルではなく公開鍵ファイルが指定されています。秘密鍵本体を指定してください"
                .to_string(),
        ),
        Some(line) if line.starts_with("PuTTY-User-Key-File-") => Err(
            "PuTTY形式(.ppk)の秘密鍵は直接読み込めません。OpenSSH形式の秘密鍵に変換してください"
                .to_string(),
        ),
        Some(line)
            if line.starts_with("-----BEGIN ")
                && line.contains("PUBLIC KEY")
                && !line.contains("PRIVATE KEY") =>
        {
            Err(
                "秘密鍵ファイルではなく公開鍵ファイルが指定されています。秘密鍵本体を指定してください"
                    .to_string(),
            )
        }
        Some(line)
            if line == "-----BEGIN OPENSSH PRIVATE KEY-----"
                || line == "-----BEGIN RSA PRIVATE KEY-----"
                || line == "-----BEGIN ENCRYPTED PRIVATE KEY-----"
                || line == "-----BEGIN PRIVATE KEY-----" =>
        {
            Ok(())
        }
        _ => Err(
            "OpenSSH/PEM形式の秘密鍵ファイルを指定してください。公開鍵ファイルは秘密鍵として使用できません"
                .to_string(),
        ),
    }
}

fn read_private_key_secret(path: &str) -> Result<(PathBuf, String), String> {
    let path = normalize_auth_path(path);
    let secret = fs::read_to_string(&path).map_err(|error| {
        format!(
            "秘密鍵ファイルを開けません: {} ({})",
            path.to_string_lossy(),
            error
        )
    })?;

    Ok((path, secret))
}

pub fn private_key_requires_passphrase(path: &str) -> Result<bool, String> {
    if path.trim().is_empty() {
        return Err("秘密鍵ファイルを指定してください".to_string());
    }

    let (_path, secret) = read_private_key_secret(path)?;
    private_key_format_hint(&secret)?;
    match decode_secret_key(&secret, None) {
        Ok(_) => Ok(false),
        Err(russh::keys::Error::KeyIsEncrypted) => Ok(true),
        Err(russh::keys::Error::CouldNotReadKey) => Err(
            "秘密鍵を読み込めません。鍵形式、パスフレーズ、またはファイル内容を確認してください"
                .to_string(),
        ),
        Err(other) => Err(format!("秘密鍵を読み込めません: {}", other)),
    }
}

#[tauri::command]
pub fn ssh_private_key_requires_passphrase(private_key_path: String) -> Result<bool, String> {
    private_key_requires_passphrase(&private_key_path)
        .map_err(|error| format!("SSH公開鍵認証エラー: {}", error))
}

fn load_private_key_for_auth(path: &str, passphrase: Option<&str>) -> Result<PrivateKey, String> {
    let (_path, secret) = read_private_key_secret(path)?;
    private_key_format_hint(&secret)?;
    decode_secret_key(&secret, passphrase).map_err(|error| match error {
        russh::keys::Error::KeyIsEncrypted => {
            "秘密鍵はパスフレーズで暗号化されています。鍵パスフレーズを入力してください".to_string()
        }
        russh::keys::Error::CouldNotReadKey => {
            "秘密鍵を読み込めません。鍵形式、パスフレーズ、またはファイル内容を確認してください"
                .to_string()
        }
        other => format!("秘密鍵を読み込めません: {}", other),
    })
}

async fn authenticate_ssh(
    handle: &mut russh::client::Handle<impl russh::client::Handler + Send + 'static>,
    username: &str,
    auth: SshAuthRequest,
) -> Result<(), String> {
    let (auth_result, failure_message) = match auth {
        SshAuthRequest::Password { password } => (
            handle
                .authenticate_password(username, &password)
                .await
                .map_err(|e| format!("SSH認証エラー: {}", e))?,
            "SSH認証失敗: ユーザー名またはパスワードが正しくありません",
        ),
        SshAuthRequest::PublicKey {
            private_key_path,
            key_passphrase,
        } => {
            let key = load_private_key_for_auth(&private_key_path, key_passphrase.as_deref())
                .map_err(|e| format!("SSH公開鍵認証エラー: {}", e))?;

            (
                handle
                    .authenticate_publickey(
                        username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), None),
                    )
                    .await
                    .map_err(|e| format!("SSH公開鍵認証エラー: {}", e))?,
                "SSH公開鍵認証失敗: ユーザー名、秘密鍵、公開鍵の登録状態、またはパスフレーズを確認してください",
            )
        }
    };

    if !auth_result.success() {
        return Err(failure_message.to_string());
    }

    Ok(())
}

fn host_key_error_message(result: &HostKeyCheckResult) -> String {
    match result.status {
        HostKeyCheckStatus::Unknown => format!(
            "SSHホスト鍵が未信頼です。接続前にフィンガープリントを確認してください: SHA256:{}",
            result.fingerprint
        ),
        HostKeyCheckStatus::Mismatch => format!(
            "SSHホスト鍵が一致しません。MITMの可能性があります。保存済み: {} / 受信: SHA256:{}",
            result
                .known_fingerprint
                .as_deref()
                .map(|fingerprint| format!("SHA256:{}", fingerprint))
                .unwrap_or_else(|| "不明".to_string()),
            result.fingerprint
        ),
        HostKeyCheckStatus::Trusted => "SSHホスト鍵検証エラー".to_string(),
    }
}

fn map_connect_error(error: russh::Error, verifier: &HostKeyVerifier) -> String {
    if let Some(result) = verifier.last_result() {
        if result.status != HostKeyCheckStatus::Trusted {
            return host_key_error_message(&result);
        }
    }
    format!("SSH接続エラー: {}", error)
}

fn emit_host_key_diagnostic(diagnostic: &SshDiagnostic, result: &HostKeyCheckResult) {
    diagnostic.info(format!(
        "target: host key received {} SHA256:{}",
        result.algorithm, result.fingerprint
    ));
    match result.status {
        HostKeyCheckStatus::Trusted => diagnostic.info("target: host key trusted"),
        HostKeyCheckStatus::Unknown => diagnostic.info("target: host key unknown"),
        HostKeyCheckStatus::Mismatch => diagnostic.info("target: host key mismatch"),
    }
}

fn normalize_diagnostic_role(value: Option<String>) -> &'static str {
    match value.as_deref() {
        Some("jump") => "jump",
        _ => "target",
    }
}

fn emit_host_key_diagnostic_for_role(
    diagnostic: &SshDiagnostic,
    role: &str,
    result: &HostKeyCheckResult,
) {
    diagnostic.info(format!(
        "{role}: host key received {} SHA256:{}",
        result.algorithm, result.fingerprint
    ));
    match result.status {
        HostKeyCheckStatus::Trusted => diagnostic.info(format!("{role}: host key trusted")),
        HostKeyCheckStatus::Unknown => diagnostic.info(format!("{role}: host key unknown")),
        HostKeyCheckStatus::Mismatch => diagnostic.info(format!("{role}: host key mismatch")),
    }
}

fn append_missing<T: Clone + PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn build_client_config(ssh_config: &SshConfig) -> russh::client::Config {
    let mut config = russh::client::Config::default();
    if !ssh_config.allow_legacy_algorithms {
        return config;
    }

    let default_preferred = russh::Preferred::default();
    let mut kex = default_preferred.kex.to_vec();
    append_missing(&mut kex, russh::kex::DH_G1_SHA1);
    append_missing(&mut kex, russh::kex::DH_G14_SHA1);

    let mut cipher = default_preferred.cipher.to_vec();
    append_missing(&mut cipher, russh::cipher::AES_128_CBC);
    append_missing(&mut cipher, russh::cipher::AES_192_CBC);
    append_missing(&mut cipher, russh::cipher::AES_256_CBC);
    append_missing(&mut cipher, russh::cipher::TRIPLE_DES_CBC);

    let mut mac = default_preferred.mac.to_vec();
    append_missing(&mut mac, russh::mac::HMAC_SHA1);
    append_missing(&mut mac, russh::mac::HMAC_SHA1_ETM);

    let mut key = default_preferred.key.to_vec();
    append_missing(&mut key, Algorithm::Rsa { hash: None });

    config.preferred = russh::Preferred {
        kex: Cow::Owned(kex),
        key: Cow::Owned(key),
        cipher: Cow::Owned(cipher),
        mac: Cow::Owned(mac),
        compression: default_preferred.compression,
    };
    config
}

fn load_client_config() -> Result<russh::client::Config, String> {
    let app_config = config_load()?;
    Ok(build_client_config(&app_config.ssh))
}

async fn connect_jump_profile(
    config: Arc<russh::client::Config>,
    jump_profile: SshJumpProfile,
    target_host: &str,
    target_port: u16,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
    diagnostic: Option<&SshDiagnostic>,
) -> Result<
    (
        russh::client::Handle<ProbeClientHandler>,
        russh::Channel<russh::client::Msg>,
    ),
    String,
> {
    let auth = build_auth_request(
        Some(jump_profile.auth_method.clone()),
        jump_password.unwrap_or_default(),
        jump_profile.private_key_path.clone(),
        jump_key_passphrase,
    )?;
    let jump_verifier = HostKeyVerifier::enforce(jump_profile.host.clone(), jump_profile.port);
    let handler = ProbeClientHandler {
        host_verifier: jump_verifier.clone(),
    };
    if let Some(diagnostic) = diagnostic {
        diagnostic.info("jump: connecting");
    }
    let mut handle = russh::client::connect(
        config,
        (jump_profile.host.as_str(), jump_profile.port),
        handler,
    )
    .await
    .map_err(|error| {
        if let Some(diagnostic) = diagnostic {
            diagnostic.error("error: jump SSH handshake failed");
        }
        map_connect_error(error, &jump_verifier)
    })?;

    if let Some(diagnostic) = diagnostic {
        diagnostic.info("jump: host key accepted");
        diagnostic.info("jump: authentication started");
    }

    authenticate_ssh(&mut handle, &jump_profile.username, auth)
        .await
        .map_err(|error| {
            if let Some(diagnostic) = diagnostic {
                diagnostic.error("error: jump authentication failed");
            }
            error
        })?;

    if let Some(diagnostic) = diagnostic {
        diagnostic.info("jump: authentication succeeded");
        diagnostic.info("jump: opening direct-tcpip channel");
    }

    let channel = handle
        .channel_open_direct_tcpip(target_host, u32::from(target_port), "127.0.0.1", 0)
        .await
        .map_err(|error| {
            if let Some(diagnostic) = diagnostic {
                diagnostic.error("error: jump direct-tcpip channel failed");
            }
            format!("SSH踏み台チャネルオープンエラー: {}", error)
        })?;

    if let Some(diagnostic) = diagnostic {
        diagnostic.info("jump: direct-tcpip channel opened");
    }

    Ok((handle, channel))
}

async fn run_host_key_probe(
    host: &str,
    port: u16,
    jump_profile: Option<SshJumpProfile>,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
    diagnostic: Option<SshDiagnostic>,
    role: &'static str,
) -> Result<(HostKeyCheckResult, PendingHostKey), String> {
    let config = Arc::new(load_client_config()?);
    let verifier = HostKeyVerifier::probe(host.to_string(), port);
    let handler = ProbeClientHandler {
        host_verifier: verifier.clone(),
    };

    let (handle, jump_handle) = if let Some(jump_profile) = jump_profile {
        let (jump_handle, jump_channel) = connect_jump_profile(
            config.clone(),
            jump_profile,
            host,
            port,
            jump_password,
            jump_key_passphrase,
            diagnostic.as_ref(),
        )
        .await?;
        let stream = jump_channel.into_stream();
        if let Some(diagnostic) = &diagnostic {
            diagnostic.info(format!("{role}: starting SSH handshake"));
        }
        let handle = russh::client::connect_stream(config, stream, handler)
            .await
            .map_err(|error| {
                if let Some(diagnostic) = &diagnostic {
                    diagnostic.error(format!("error: {role} SSH handshake failed"));
                }
                format!("SSH接続エラー: {}", error)
            })?;
        (handle, Some(jump_handle))
    } else {
        if let Some(diagnostic) = &diagnostic {
            diagnostic.info(format!("{role}: starting SSH handshake"));
        }
        let handle = russh::client::connect(config, (host, port), handler)
            .await
            .map_err(|error| {
                if let Some(diagnostic) = &diagnostic {
                    diagnostic.error(format!("error: {role} SSH handshake failed"));
                }
                format!("SSH接続エラー: {}", error)
            })?;
        (handle, None)
    };

    let _ = handle
        .disconnect(Disconnect::ByApplication, "Host key probe completed", "en")
        .await;
    if let Some(jump_handle) = jump_handle {
        let _ = jump_handle
            .disconnect(Disconnect::ByApplication, "Host key probe completed", "en")
            .await;
    }

    let result = verifier
        .last_result()
        .ok_or_else(|| "SSHホスト鍵を取得できませんでした".to_string())?;
    let observed_key = verifier
        .observed_key()
        .ok_or_else(|| "SSHホスト鍵を取得できませんでした".to_string())?;

    if let Some(diagnostic) = &diagnostic {
        emit_host_key_diagnostic_for_role(diagnostic, role, &result);
    }

    Ok((result, observed_key))
}

#[cfg(not(test))]
pub async fn verify_trusted_host_key(host: &str, port: u16) -> Result<(), String> {
    let (result, _) = run_host_key_probe(host, port, None, None, None, None, "target").await?;
    if result.status == HostKeyCheckStatus::Trusted {
        Ok(())
    } else {
        Err(host_key_error_message(&result))
    }
}

#[cfg(not(test))]
pub async fn verify_trusted_host_key_via_jump(
    host: &str,
    port: u16,
    jump_profile: SshJumpProfile,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
) -> Result<(), String> {
    let (result, _) = run_host_key_probe(
        host,
        port,
        Some(jump_profile),
        jump_password,
        jump_key_passphrase,
        None,
        "target",
    )
    .await?;
    if result.status == HostKeyCheckStatus::Trusted {
        Ok(())
    } else {
        Err(host_key_error_message(&result))
    }
}

#[tauri::command]
pub async fn ssh_probe_host_key(
    app: AppHandle,
    state: tauri::State<'_, SshState>,
    host: String,
    port: u16,
    jump_profile_id: Option<String>,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
    request_id: Option<String>,
    diagnostic_role: Option<String>,
) -> Result<HostKeyCheckResult, String> {
    let config = config_load()?;
    let jump_profile = resolve_jump_profile(&config, jump_profile_id.as_deref(), None)?;
    let diagnostic = SshDiagnostic::new(&app, request_id);
    let role = normalize_diagnostic_role(diagnostic_role);
    let (result, pending_key) = run_host_key_probe(
        &host,
        port,
        jump_profile,
        jump_password,
        jump_key_passphrase,
        Some(diagnostic),
        role,
    )
    .await?;
    state
        .pending_host_keys
        .lock()
        .await
        .insert(endpoint_cache_key(&host, port), pending_key);
    Ok(result)
}

#[tauri::command]
pub async fn ssh_trust_host_key(
    state: tauri::State<'_, SshState>,
    host: String,
    port: u16,
    replace: bool,
) -> Result<(), String> {
    let endpoint = endpoint_cache_key(&host, port);
    let pending_key = {
        let pending = state.pending_host_keys.lock().await;
        pending.get(&endpoint).cloned().ok_or_else(|| {
            "直前に取得したSSHホスト鍵が見つかりません。もう一度接続してください。".to_string()
        })?
    };

    write_trusted_host(&host, port, &pending_key.key, replace)?;
    state.pending_host_keys.lock().await.remove(&endpoint);
    Ok(())
}

/// Connect to SSH server with password authentication
#[tauri::command]
pub async fn ssh_connect(
    app: AppHandle,
    state: tauri::State<'_, SshState>,
    terminals: tauri::State<'_, TerminalControlState>,
    workspace: tauri::State<'_, WorkspaceState>,
    logger: tauri::State<'_, LoggerState>,
    host: String,
    port: u16,
    username: String,
    password: String,
    auth_method: Option<String>,
    private_key_path: Option<String>,
    key_passphrase: Option<String>,
    jump_profile_id: Option<String>,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
    cols: u32,
    rows: u32,
    encoding: Option<String>,
    request_id: Option<String>,
) -> Result<SshConnectResult, String> {
    connect(
        &app,
        &state,
        &terminals,
        &workspace,
        Some(&logger),
        SshConnectOptions {
            host,
            port,
            username,
            password,
            auth_method,
            private_key_path,
            key_passphrase,
            jump_profile_id,
            jump_password,
            jump_key_passphrase,
            cols,
            rows,
            encoding,
            request_id,
        },
    )
    .await
}

pub async fn connect(
    app: &AppHandle,
    state: &SshState,
    terminals: &TerminalControlState,
    workspace: &WorkspaceState,
    logger_state: Option<&LoggerState>,
    options: SshConnectOptions,
) -> Result<SshConnectResult, String> {
    let session_id = Uuid::new_v4().to_string();
    let diagnostic = SshDiagnostic::new(app, options.request_id.clone());
    let auth = build_auth_request(
        options.auth_method.clone(),
        options.password.clone(),
        options.private_key_path.clone(),
        options.key_passphrase.clone(),
    )?;
    let app_config = config_load()?;
    let jump_profile = resolve_jump_profile(&app_config, options.jump_profile_id.as_deref(), None)?;
    let config = Arc::new(build_client_config(&app_config.ssh));
    let host_verifier = HostKeyVerifier::enforce(options.host.clone(), options.port);
    let (read_tx, read_rx) = mpsc::channel::<SshReadRequest>(SSH_READ_QUEUE_CAPACITY);
    let read_drop_state = Arc::new(StdMutex::new(SshReadDropState::default()));
    let handler = SshClientHandler {
        app: app.clone(),
        session_id: session_id.clone(),
        sessions: state.sessions.clone(),
        host_verifier: host_verifier.clone(),
        terminals: terminals.clone(),
        workspace: workspace.clone(),
        logger: logger_state.cloned(),
        read_tx,
        read_drop_state,
    };

    let (mut handle, jump_handle) = if let Some(jump_profile) = jump_profile {
        let (jump_handle, jump_channel) = connect_jump_profile(
            config.clone(),
            jump_profile,
            &options.host,
            options.port,
            options.jump_password.clone(),
            options.jump_key_passphrase.clone(),
            Some(&diagnostic),
        )
        .await?;
        let stream = jump_channel.into_stream();
        diagnostic.info("target: starting SSH handshake");
        let handle = russh::client::connect_stream(config, stream, handler)
            .await
            .map_err(|error| {
                if let Some(result) = host_verifier.last_result() {
                    emit_host_key_diagnostic(&diagnostic, &result);
                }
                diagnostic.error("error: target SSH handshake failed");
                map_connect_error(error, &host_verifier)
            })?;
        (handle, Some(jump_handle))
    } else {
        diagnostic.info("target: starting SSH handshake");
        let handle = russh::client::connect(config, (options.host.as_str(), options.port), handler)
            .await
            .map_err(|error| {
                if let Some(result) = host_verifier.last_result() {
                    emit_host_key_diagnostic(&diagnostic, &result);
                }
                diagnostic.error("error: target SSH handshake failed");
                map_connect_error(error, &host_verifier)
            })?;
        (handle, None)
    };

    if let Some(result) = host_verifier.last_result() {
        emit_host_key_diagnostic(&diagnostic, &result);
    }
    diagnostic.info("target: authentication started");
    authenticate_ssh(&mut handle, &options.username, auth)
        .await
        .map_err(|error| {
            diagnostic.error("error: target authentication failed");
            error
        })?;
    diagnostic.info("target: authentication succeeded");

    diagnostic.info("target: opening session channel");
    let channel = handle.channel_open_session().await.map_err(|e| {
        diagnostic.error("error: target session channel failed");
        format!("SSHチャネルオープンエラー: {}", e)
    })?;

    diagnostic.info("target: requesting pty");
    channel
        .request_pty(
            false,
            "xterm-256color",
            options.cols,
            options.rows,
            0,
            0,
            &[],
        )
        .await
        .map_err(|_| {
            diagnostic.error("error: target pty request failed");
            "PTYリクエストエラー".to_string()
        })?;

    diagnostic.info("target: requesting shell");
    channel.request_shell(false).await.map_err(|_| {
        diagnostic.error("error: target shell request failed");
        "シェルリクエストエラー".to_string()
    })?;

    let (mut channel_read_half, channel_write_half) = channel.split();
    tokio::spawn(async move { while channel_read_half.wait().await.is_some() {} });
    spawn_ssh_read_processor(app, &session_id, terminals.clone(), read_rx);

    let session = SshSession {
        handle,
        channel: Arc::new(Mutex::new(channel_write_half)),
        jump_handle,
    };

    state
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), Arc::new(Mutex::new(session)));

    terminals
        .register_session_with_encoding(
            session_id.clone(),
            TerminalProtocol::Ssh,
            format!("{}:{}", options.host, options.port),
            options.encoding,
        )
        .await;

    let _ = app.emit("ssh://connected", &session_id);
    diagnostic.info("target: session ready");

    Ok(SshConnectResult { session_id })
}

/// Write data to SSH session
#[tauri::command]
pub async fn ssh_write(
    state: tauri::State<'_, SshState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    write_data(&state, &session_id, data).await
}

pub async fn write_data(state: &SshState, session_id: &str, data: String) -> Result<(), String> {
    let session = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(session_id)
            .ok_or("セッションが見つかりません")?
            .clone()
    };

    let channel = session.lock().await.channel.clone();
    channel
        .lock()
        .await
        .data_bytes(data.into_bytes())
        .await
        .map_err(|_| "SSH送信エラー".to_string())?;

    Ok(())
}

/// Resize SSH terminal
#[tauri::command]
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
            .ok_or("セッションが見つかりません")?
            .clone()
    };

    let channel = session.lock().await.channel.clone();
    channel
        .lock()
        .await
        .window_change(cols, rows, 0, 0)
        .await
        .map_err(|_| "SSHリサイズエラー".to_string())?;

    Ok(())
}

/// Disconnect SSH session
#[tauri::command]
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;

    fn preferred_names<T: AsRef<str>>(items: &[T]) -> Vec<&str> {
        items.iter().map(|item| item.as_ref()).collect()
    }

    fn temp_known_hosts_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exaterm-ssh-verifier-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("known_hosts")
    }

    fn read_test_key(base64: &str) -> PublicKey {
        russh::keys::parse_public_key_base64(base64).unwrap()
    }

    fn write_temp_private_key(contents: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exaterm-ssh-key-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("id_ed25519");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn diagnostic_event_name_scopes_to_request_id() {
        assert_eq!(
            ssh_diagnostic_event_name("request-1"),
            "ssh://connect-diagnostic/request-1"
        );
    }

    #[test]
    fn read_overflow_event_name_scopes_to_session_id() {
        assert_eq!(
            ssh_read_overflow_event_name("session-1"),
            "ssh://read-overflow/session-1"
        );
    }

    #[test]
    fn read_drop_state_reports_first_drop_only_until_interval() {
        let state = Arc::new(StdMutex::new(SshReadDropState::default()));

        assert_eq!(
            record_ssh_read_drop(&state, 32),
            Some(SshReadDropNotice {
                dropped_chunks: 1,
                dropped_bytes: 32,
            })
        );
        assert_eq!(record_ssh_read_drop(&state, 64), None);

        let state = state.lock().unwrap();
        assert_eq!(state.dropped_chunks, 2);
        assert_eq!(state.dropped_bytes, 96);
    }

    #[test]
    fn read_drop_state_reports_every_notice_interval() {
        let state = Arc::new(StdMutex::new(SshReadDropState::default()));
        let mut last_notice = None;

        for _ in 0..SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS {
            last_notice = record_ssh_read_drop(&state, 1);
        }

        assert_eq!(
            last_notice,
            Some(SshReadDropNotice {
                dropped_chunks: SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS,
                dropped_bytes: SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS,
            })
        );
    }

    #[test]
    fn diagnostic_role_only_allows_known_roles() {
        assert_eq!(normalize_diagnostic_role(Some("jump".into())), "jump");
        assert_eq!(normalize_diagnostic_role(Some("target".into())), "target");
        assert_eq!(
            normalize_diagnostic_role(Some("host.example.com".into())),
            "target"
        );
        assert_eq!(normalize_diagnostic_role(None), "target");
    }

    fn generate_temp_private_key(passphrase: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exaterm-ssh-keygen-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("id_ed25519");
        let status = Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-N", passphrase, "-f"])
            .arg(&path)
            .arg("-q")
            .status()
            .unwrap_or_else(|error| panic!("ssh-keygen is required for this test: {error}"));
        assert!(status.success(), "ssh-keygen failed with status {status}");
        path
    }

    #[test]
    fn verifier_rejects_unknown_keys_in_enforce_mode() {
        let verifier = HostKeyVerifier::with_path(
            "example.com".to_string(),
            22,
            HostKeyVerificationMode::Enforce,
            temp_known_hosts_path(),
        );
        let key =
            read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ");

        let allowed = verifier.check_key(&key).unwrap();
        assert!(!allowed);
        assert_eq!(
            verifier.last_result().unwrap().status,
            HostKeyCheckStatus::Unknown
        );
    }

    #[test]
    fn verifier_rejects_mismatched_keys_in_enforce_mode() {
        let known_hosts_path = temp_known_hosts_path();
        let stored_key =
            read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAILIG2T/B0l0gaqj3puu510tu9N1OkQ4znY3LYuEm5zCF");
        crate::ssh_known_hosts::write_trusted_host_with_path(
            "example.com",
            22,
            &stored_key,
            false,
            &known_hosts_path,
        )
        .unwrap();

        let verifier = HostKeyVerifier::with_path(
            "example.com".to_string(),
            22,
            HostKeyVerificationMode::Enforce,
            known_hosts_path.clone(),
        );
        let new_key =
            read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ");

        let allowed = verifier.check_key(&new_key).unwrap();
        assert!(!allowed);
        assert_eq!(
            verifier.last_result().unwrap().status,
            HostKeyCheckStatus::Mismatch
        );

        let _ = fs::remove_file(known_hosts_path);
    }

    #[test]
    fn default_client_config_keeps_russh_defaults() {
        let config = build_client_config(&SshConfig::default());
        let default = russh::client::Config::default();

        assert_eq!(
            preferred_names(config.preferred.kex.as_ref()),
            preferred_names(default.preferred.kex.as_ref())
        );
        assert_eq!(
            preferred_names(config.preferred.cipher.as_ref()),
            preferred_names(default.preferred.cipher.as_ref())
        );
        assert_eq!(
            preferred_names(config.preferred.mac.as_ref()),
            preferred_names(default.preferred.mac.as_ref())
        );
        assert_eq!(
            preferred_names(config.preferred.key.as_ref()),
            preferred_names(default.preferred.key.as_ref())
        );
    }

    #[test]
    fn legacy_client_config_appends_legacy_algorithms() {
        let config = build_client_config(&SshConfig {
            allow_legacy_algorithms: true,
        });

        let kex = preferred_names(config.preferred.kex.as_ref());
        let cipher = preferred_names(config.preferred.cipher.as_ref());
        let mac = preferred_names(config.preferred.mac.as_ref());
        let key = preferred_names(config.preferred.key.as_ref());

        assert!(kex.contains(&"diffie-hellman-group1-sha1"));
        assert!(kex.contains(&"diffie-hellman-group14-sha1"));
        assert!(cipher.contains(&"aes128-cbc"));
        assert!(cipher.contains(&"aes192-cbc"));
        assert!(cipher.contains(&"aes256-cbc"));
        assert!(cipher.contains(&"3des-cbc"));
        assert!(mac.contains(&"hmac-sha1"));
        assert!(mac.contains(&"hmac-sha1-etm@openssh.com"));
        assert!(key.contains(&"ssh-rsa"));
    }

    #[test]
    fn auth_request_defaults_to_password() {
        let request = build_auth_request(None, "secret".to_string(), None, None).unwrap();

        assert_eq!(
            request,
            SshAuthRequest::Password {
                password: "secret".to_string()
            }
        );
    }

    #[test]
    fn public_key_auth_requires_private_key_path() {
        let error = build_auth_request(
            Some("public_key".to_string()),
            String::new(),
            Some("  ".to_string()),
            None,
        )
        .unwrap_err();

        assert!(error.contains("秘密鍵ファイル"));
    }

    #[test]
    fn public_key_auth_trims_path_and_empty_passphrase() {
        let request = build_auth_request(
            Some("public_key".to_string()),
            String::new(),
            Some(" C:\\Users\\me\\.ssh\\id_ed25519 ".to_string()),
            Some("  ".to_string()),
        )
        .unwrap();

        assert_eq!(
            request,
            SshAuthRequest::PublicKey {
                private_key_path: "C:\\Users\\me\\.ssh\\id_ed25519".to_string(),
                key_passphrase: None,
            }
        );
    }

    #[test]
    fn auth_path_strips_quotes() {
        assert_eq!(
            normalize_auth_path("\"C:\\Users\\me\\.ssh\\id_ed25519\""),
            PathBuf::from("C:\\Users\\me\\.ssh\\id_ed25519")
        );
    }

    #[test]
    fn private_key_hint_rejects_public_key_files() {
        let error =
            private_key_format_hint("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEexample user@example\n")
                .unwrap_err();

        assert!(error.contains("公開鍵ファイル"));
    }

    #[test]
    fn private_key_hint_rejects_putty_keys() {
        let error = private_key_format_hint("PuTTY-User-Key-File-3: ssh-ed25519\n").unwrap_err();

        assert!(error.contains("PuTTY形式"));
    }

    #[test]
    fn private_key_requires_passphrase_detects_unencrypted_keys() {
        let path = generate_temp_private_key("");

        let requires_passphrase = private_key_requires_passphrase(path.to_str().unwrap()).unwrap();

        assert!(!requires_passphrase);
    }

    #[test]
    fn private_key_requires_passphrase_detects_encrypted_keys() {
        let path = generate_temp_private_key("secret");

        let requires_passphrase = private_key_requires_passphrase(path.to_str().unwrap()).unwrap();

        assert!(requires_passphrase);
    }

    #[test]
    fn private_key_requires_passphrase_rejects_public_key_files() {
        let path =
            write_temp_private_key("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEexample user@example\n");

        let error = private_key_requires_passphrase(path.to_str().unwrap()).unwrap_err();

        assert!(error.contains("公開鍵ファイル"));
    }

    #[test]
    fn private_key_requires_passphrase_rejects_empty_path() {
        let error = private_key_requires_passphrase("  ").unwrap_err();

        assert!(error.contains("秘密鍵ファイル"));
    }
}
