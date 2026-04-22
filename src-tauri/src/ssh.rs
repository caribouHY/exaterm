use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use russh::*;
use russh_keys::key::PublicKey;
use uuid::Uuid;

/// SSH session state shared across async tasks
struct SshSession {
    handle: russh::client::Handle<SshClientHandler>,
    channel: russh::Channel<russh::client::Msg>,
    session_id: String,
}

/// Global SSH session store
pub struct SshState {
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<SshSession>>>>>,
}

impl SshState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// russh client handler — receives data from server
struct SshClientHandler {
    app: AppHandle,
    session_id: String,
}

#[async_trait::async_trait]
impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // Accept all host keys for now (TODO: known_hosts verification)
        Ok(true)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        let _ = self.app.emit(
            &format!("ssh://data/{}", self.session_id),
            data.to_vec(),
        );
        Ok(())
    }

    async fn extended_data(
        &mut self,
        _channel: ChannelId,
        _ext: u32,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        let _ = self.app.emit(
            &format!("ssh://error/{}", self.session_id),
            data.to_vec(),
        );
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConnectResult {
    pub session_id: String,
}

/// Connect to SSH server with password authentication
#[tauri::command]
pub async fn ssh_connect(
    app: AppHandle,
    state: tauri::State<'_, SshState>,
    host: String,
    port: u16,
    username: String,
    password: String,
    cols: u32,
    rows: u32,
) -> Result<SshConnectResult, String> {
    let session_id = Uuid::new_v4().to_string();

    let config = russh::client::Config::default();
    let handler = SshClientHandler {
        app: app.clone(),
        session_id: session_id.clone(),
    };

    let mut handle = russh::client::connect(Arc::new(config), (host.as_str(), port), handler)
        .await
        .map_err(|e| format!("SSH接続エラー: {}", e))?;

    let auth_result = handle
        .authenticate_password(&username, &password)
        .await
        .map_err(|e| format!("SSH認証エラー: {}", e))?;

    if !auth_result {
        return Err("SSH認証失敗: ユーザー名またはパスワードが正しくありません".to_string());
    }

    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("SSHチャネルオープンエラー: {}", e))?;

    channel
        .request_pty(
            false,
            "xterm-256color",
            cols,
            rows,
            0,
            0,
            &[],
        )
        .await
        .map_err(|_| "PTYリクエストエラー".to_string())?;

    channel
        .request_shell(false)
        .await
        .map_err(|_| "シェルリクエストエラー".to_string())?;

    let session = SshSession {
        handle,
        channel,
        session_id: session_id.clone(),
    };

    state
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), Arc::new(Mutex::new(session)));

    let _ = app.emit("ssh://connected", &session_id);

    Ok(SshConnectResult { session_id })
}

/// Write data to SSH session
#[tauri::command]
pub async fn ssh_write(
    state: tauri::State<'_, SshState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&session_id)
        .ok_or("セッションが見つかりません")?
        .clone();

    let mut session = session.lock().await;
    session
        .channel
        .data(data.as_bytes())
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
    let sessions = state.sessions.lock().await;
    let session = sessions
        .get(&session_id)
        .ok_or("セッションが見つかりません")?
        .clone();

    let mut session = session.lock().await;
    session
        .channel
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
    session_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    if let Some(session) = sessions.remove(&session_id) {
        let mut session = session.lock().await;
        let _ = session
            .handle
            .disconnect(Disconnect::ByApplication, "User disconnected", "en")
            .await;
    }
    let _ = app.emit("ssh://disconnected", &session_id);
    Ok(())
}

