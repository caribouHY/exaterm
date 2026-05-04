use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use russh::*;
use russh_keys::decode_secret_key;
use russh_keys::key::{KeyPair, PublicKey};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::{config_load, SshConfig};
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
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<SshSession>>>>>,
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

    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        self.mark_disconnected().await;
        Ok(())
    }

    async fn disconnected(
        &mut self,
        reason: russh::client::DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        self.mark_disconnected().await;
        match reason {
            russh::client::DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            russh::client::DisconnectReason::Error(error) => Err(error),
        }
    }
}

impl SshClientHandler {
    async fn mark_disconnected(&self) {
        let was_connected = self
            .sessions
            .lock()
            .await
            .remove(&self.session_id)
            .is_some();
        if was_connected {
            let _ = self.app.emit("ssh://disconnected", &self.session_id);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConnectResult {
    pub session_id: String,
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

fn load_private_key_for_auth(path: &str, passphrase: Option<&str>) -> Result<KeyPair, String> {
    let path = normalize_auth_path(path);
    let secret = fs::read_to_string(&path).map_err(|error| {
        format!(
            "秘密鍵ファイルを開けません: {} ({})",
            path.to_string_lossy(),
            error
        )
    })?;

    private_key_format_hint(&secret)?;
    decode_secret_key(&secret, passphrase).map_err(|error| match error {
        russh_keys::Error::KeyIsEncrypted => {
            "秘密鍵はパスフレーズで暗号化されています。鍵パスフレーズを入力してください".to_string()
        }
        russh_keys::Error::CouldNotReadKey => {
            "秘密鍵を読み込めません。鍵形式、パスフレーズ、またはファイル内容を確認してください"
                .to_string()
        }
        other => format!("秘密鍵を読み込めません: {}", other),
    })
}

async fn authenticate_ssh(
    handle: &mut russh::client::Handle<SshClientHandler>,
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
                    .authenticate_publickey(username, Arc::new(key))
                    .await
                    .map_err(|e| format!("SSH公開鍵認証エラー: {}", e))?,
                "SSH公開鍵認証失敗: ユーザー名、秘密鍵、公開鍵の登録状態、またはパスフレーズを確認してください",
            )
        }
    };

    if !auth_result {
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

fn append_missing<T: Copy + PartialEq>(items: &mut Vec<T>, item: T) {
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
    append_missing(&mut key, russh::keys::key::SSH_RSA);

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

async fn run_host_key_probe(
    host: &str,
    port: u16,
) -> Result<(HostKeyCheckResult, PendingHostKey), String> {
    let config = load_client_config()?;
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
    auth_method: Option<String>,
    private_key_path: Option<String>,
    key_passphrase: Option<String>,
    cols: u32,
    rows: u32,
) -> Result<SshConnectResult, String> {
    let session_id = Uuid::new_v4().to_string();
    let auth = build_auth_request(auth_method, password, private_key_path, key_passphrase)?;
    let config = load_client_config()?;
    let host_verifier = HostKeyVerifier::enforce(host.clone(), port);
    let handler = SshClientHandler {
        app: app.clone(),
        session_id: session_id.clone(),
        sessions: state.sessions.clone(),
        host_verifier: host_verifier.clone(),
    };

    let mut handle = russh::client::connect(Arc::new(config), (host.as_str(), port), handler)
        .await
        .map_err(|error| map_connect_error(error, &host_verifier))?;

    authenticate_ssh(&mut handle, &username, auth).await?;

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
    let session = state.sessions.lock().await.remove(&session_id);
    if let Some(session) = session {
        let session = session.lock().await;
        let _ = session.channel.eof().await;
        let _ = session.channel.close().await;
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

    fn preferred_names<T: AsRef<str>>(items: &[T]) -> Vec<&str> {
        items.iter().map(|item| item.as_ref()).collect()
    }

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
}
