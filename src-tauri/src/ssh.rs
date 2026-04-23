use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use russh::*;
use russh_keys::key::PublicKey;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::ssh_known_hosts::{
    endpoint_cache_key, inspect_host_key_with_path, known_hosts_path, write_trusted_host,
    HostKeyCheckResult, HostKeyCheckStatus,
};

/// SSH session state shared across async tasks
struct SshSession {
    handle: russh::client::Handle<SshClientHandler>,
    channel: russh::Channel<russh::client::Msg>,
}

#[derive(Clone)]
struct PendingHostKey {
    key: PublicKey,
}

/// Global SSH session store
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
    host_verifier: HostKeyVerifier,
}

struct ProbeClientHandler {
    host_verifier: HostKeyVerifier,
}

#[async_trait::async_trait]
impl russh::client::Handler for ProbeClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        self.host_verifier.check_key(server_public_key)
    }
}

#[async_trait::async_trait]
impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        self.host_verifier.check_key(server_public_key)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        let _ = self
            .app
            .emit(&format!("ssh://data/{}", self.session_id), data.to_vec());
        Ok(())
    }

    async fn extended_data(
        &mut self,
        _channel: ChannelId,
        _ext: u32,
        data: &[u8],
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        let _ = self
            .app
            .emit(&format!("ssh://error/{}", self.session_id), data.to_vec());
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConnectResult {
    pub session_id: String,
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

async fn run_host_key_probe(
    host: &str,
    port: u16,
) -> Result<(HostKeyCheckResult, PendingHostKey), String> {
    let config = russh::client::Config::default();
    let verifier = HostKeyVerifier::probe(host.to_string(), port);
    let handler = ProbeClientHandler {
        host_verifier: verifier.clone(),
    };

    let handle = russh::client::connect(Arc::new(config), (host, port), handler)
        .await
        .map_err(|error| format!("SSH接続エラー: {}", error))?;

    let _ = handle
        .disconnect(Disconnect::ByApplication, "Host key probe completed", "en")
        .await;

    let result = verifier
        .last_result()
        .ok_or_else(|| "SSHホスト鍵を取得できませんでした".to_string())?;
    let observed_key = verifier
        .observed_key()
        .ok_or_else(|| "SSHホスト鍵を取得できませんでした".to_string())?;

    Ok((result, observed_key))
}

#[tauri::command]
pub async fn ssh_probe_host_key(
    state: tauri::State<'_, SshState>,
    host: String,
    port: u16,
) -> Result<HostKeyCheckResult, String> {
    let (result, pending_key) = run_host_key_probe(&host, port).await?;
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
    host: String,
    port: u16,
    username: String,
    password: String,
    cols: u32,
    rows: u32,
) -> Result<SshConnectResult, String> {
    let session_id = Uuid::new_v4().to_string();
    let config = russh::client::Config::default();
    let host_verifier = HostKeyVerifier::enforce(host.clone(), port);
    let handler = SshClientHandler {
        app: app.clone(),
        session_id: session_id.clone(),
        host_verifier: host_verifier.clone(),
    };

    let mut handle = russh::client::connect(Arc::new(config), (host.as_str(), port), handler)
        .await
        .map_err(|error| map_connect_error(error, &host_verifier))?;

    let auth_result = handle
        .authenticate_password(&username, &password)
        .await
        .map_err(|e| format!("SSH認証エラー: {}", e))?;

    if !auth_result {
        return Err("SSH認証失敗: ユーザー名またはパスワードが正しくありません".to_string());
    }

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("SSHチャネルオープンエラー: {}", e))?;

    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await
        .map_err(|_| "PTYリクエストエラー".to_string())?;

    channel
        .request_shell(false)
        .await
        .map_err(|_| "シェルリクエストエラー".to_string())?;

    let session = SshSession { handle, channel };

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

    let session = session.lock().await;
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

    let session = session.lock().await;
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
        let session = session.lock().await;
        let _ = session
            .handle
            .disconnect(Disconnect::ByApplication, "User disconnected", "en")
            .await;
    }
    let _ = app.emit("ssh://disconnected", &session_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_known_hosts_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exaterm-ssh-verifier-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("known_hosts")
    }

    fn read_test_key(base64: &str) -> PublicKey {
        russh_keys::parse_public_key_base64(base64).unwrap()
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
}
