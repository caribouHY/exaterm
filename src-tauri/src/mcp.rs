use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use tokio::time;
#[cfg(not(test))]
use uuid::Uuid;

use crate::config::{self, AppConfig, McpConfig, SavedConnection};
#[cfg(not(test))]
use crate::logger::{self, LoggerState};
use crate::serial::{self, SerialState};
use crate::ssh::{self, SshState};
use crate::telnet::{self, TelnetState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};

const DEFAULT_READ_CHARS: usize = 2_000;
const MAX_READ_CHARS: usize = 20_000;
const MAX_INPUT_CHARS: usize = 20_000;
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 10_000;
const MAX_WAIT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_SETTLE_MS: u64 = 250;
const MAX_SETTLE_MS: u64 = 5_000;
const DEFAULT_CONNECT_COLS: u32 = 120;
const DEFAULT_CONNECT_ROWS: u32 = 30;
const MAX_CONNECT_DIMENSION: u32 = 1_000;
const DEFAULT_SERIAL_BAUD_RATE: u32 = 9_600;
const DEFAULT_SERIAL_DATA_BITS: u8 = 8;
const DEFAULT_SERIAL_STOP_BITS: u8 = 1;
#[cfg(not(test))]
const CREDENTIAL_REQUEST_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

#[derive(Clone)]
pub struct McpRuntime {
    pub config: McpConfig,
    #[cfg(not(test))]
    pub app: Option<AppHandle>,
    pub terminals: TerminalControlState,
    pub ssh: SshState,
    pub serial: SerialState,
    pub telnet: TelnetState,
    #[cfg(not(test))]
    pub logger: Option<LoggerState>,
    #[cfg(not(test))]
    pub credentials: Option<McpCredentialState>,
}

#[derive(Clone, Default)]
pub struct McpCredentialState {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Option<String>>>>>,
}

impl McpCredentialState {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(test))]
    async fn request_ssh_credential(
        &self,
        app: &AppHandle,
        mut payload: McpCredentialRequestPayload,
    ) -> Result<Option<String>, String> {
        let request_id = Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), sender);
        payload.request_id = request_id.clone();

        if let Err(error) = app.emit("mcp://credential-request", &payload) {
            self.pending.lock().await.remove(&request_id);
            return Err(format!("MCP認証入力リクエスト送信エラー: {error}"));
        }

        match time::timeout(
            Duration::from_millis(CREDENTIAL_REQUEST_TIMEOUT_MS),
            receiver,
        )
        .await
        {
            Ok(Ok(credential)) => Ok(credential),
            Ok(Err(_)) => Err("MCP認証入力リクエストが完了しませんでした".into()),
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err("MCP認証入力がタイムアウトしました".into())
            }
        }
    }

    async fn submit(&self, request_id: String, credential: Option<String>) -> Result<(), String> {
        let sender = self
            .pending
            .lock()
            .await
            .remove(&request_id)
            .ok_or_else(|| "MCP認証入力リクエストが見つかりません".to_string())?;
        sender
            .send(credential)
            .map_err(|_| "MCP認証入力リクエストはすでに終了しています".to_string())
    }
}

#[tauri::command]
pub async fn mcp_credential_submit(
    state: tauri::State<'_, McpCredentialState>,
    request_id: String,
    credential: Option<String>,
) -> Result<(), String> {
    state.submit(request_id, credential).await
}

#[derive(Clone)]
struct McpTerminalService {
    runtime: McpRuntime,
}

impl McpTerminalService {
    fn new(runtime: McpRuntime) -> Self {
        Self { runtime }
    }

    async fn list_terminal_sessions(&self) -> Result<Value, McpError> {
        let sessions = self.runtime.terminals.list_sessions().await;
        Ok(json!({
            "sessions": sessions,
        }))
    }

    fn ensure_connect_enabled(&self) -> Result<(), McpError> {
        if self.runtime.config.connect_enabled {
            Ok(())
        } else {
            Err(invalid_params(
                "MCP新規接続は無効です。mcp.connect_enabled=true にして再起動してください",
            ))
        }
    }

    async fn list_connection_profiles(&self) -> Result<Value, McpError> {
        self.ensure_connect_enabled()?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        Ok(json!({
            "profiles": list_connection_profiles_from_config(&config),
        }))
    }

    async fn connect_saved_profile(
        &self,
        args: ConnectSavedProfileArgs,
    ) -> Result<Value, McpError> {
        self.ensure_connect_enabled()?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        let prepared = prepare_saved_profile_connection(&config, args).map_err(invalid_params)?;
        connect_prepared_profile(&self.runtime, &config, prepared).await
    }

    async fn list_serial_ports(&self) -> Result<Value, McpError> {
        self.ensure_connect_enabled()?;
        let ports = serial::serial_list_ports().map_err(internal_error)?;
        Ok(json!({
            "ports": ports,
        }))
    }

    async fn connect_serial_console(
        &self,
        args: ConnectSerialConsoleArgs,
    ) -> Result<Value, McpError> {
        self.ensure_connect_enabled()?;
        let ports = serial::serial_list_ports().map_err(internal_error)?;
        let prepared = prepare_serial_console_connection(args, &ports).map_err(invalid_params)?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        connect_prepared_serial_console(&self.runtime, &config, prepared).await
    }

    async fn read_terminal_output(&self, args: ReadTerminalOutputArgs) -> Result<Value, McpError> {
        let snapshot = self
            .runtime
            .terminals
            .read_output(&args.session_id, normalize_max_chars(args.max_chars))
            .await
            .map_err(invalid_params)?;

        Ok(json!(snapshot))
    }

    async fn read_terminal_output_delta(
        &self,
        args: ReadTerminalOutputDeltaArgs,
    ) -> Result<Value, McpError> {
        let snapshot = self
            .runtime
            .terminals
            .read_output_delta(
                &args.session_id,
                args.cursor,
                normalize_max_chars(args.max_chars),
            )
            .await
            .map_err(invalid_params)?;

        Ok(json!(snapshot))
    }

    async fn wait_terminal_output(&self, args: WaitTerminalOutputArgs) -> Result<Value, McpError> {
        let start_cursor = match args.cursor {
            Some(cursor) => cursor,
            None => self
                .runtime
                .terminals
                .cursor(&args.session_id)
                .await
                .map_err(invalid_params)?,
        };
        let contains = args.contains.filter(|value| !value.is_empty());

        wait_for_terminal_output(
            &self.runtime.terminals,
            &args.session_id,
            start_cursor,
            contains.as_deref(),
            normalize_max_chars(args.max_chars),
            normalize_timeout_ms(args.timeout_ms),
        )
        .await
    }

    async fn send_terminal_input(&self, args: SendTerminalInputArgs) -> Result<Value, McpError> {
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, args.data).await?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
        }))
    }

    async fn run_terminal_command(&self, args: RunTerminalCommandArgs) -> Result<Value, McpError> {
        if args.command.trim().is_empty() {
            return Err(invalid_params("送信するコマンドが空です"));
        }
        if args.command.chars().count() > MAX_INPUT_CHARS {
            return Err(invalid_params(format!(
                "コマンドは{}文字以内で指定してください",
                MAX_INPUT_CHARS
            )));
        }

        let start_cursor = self
            .runtime
            .terminals
            .cursor(&args.session_id)
            .await
            .map_err(invalid_params)?;
        let data = if args.append_newline.unwrap_or(true) {
            format!("{}\n", args.command)
        } else {
            args.command.clone()
        };
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, data).await?;

        let max_chars = normalize_max_chars(args.max_chars);
        let wait_result = wait_for_terminal_output(
            &self.runtime.terminals,
            &args.session_id,
            start_cursor,
            args.wait_contains
                .as_deref()
                .filter(|value| !value.is_empty()),
            max_chars,
            normalize_timeout_ms(args.timeout_ms),
        )
        .await?;

        let settle_ms = args
            .settle_ms
            .unwrap_or(DEFAULT_SETTLE_MS)
            .clamp(0, MAX_SETTLE_MS);
        if wait_result["timed_out"] == false && settle_ms > 0 {
            time::sleep(Duration::from_millis(settle_ms)).await;
        }

        let snapshot = self
            .runtime
            .terminals
            .read_output_delta(&args.session_id, start_cursor, max_chars)
            .await
            .map_err(invalid_params)?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
            "matched": wait_result["matched"],
            "timed_out": wait_result["timed_out"],
            "output": snapshot.output,
            "truncated": snapshot.truncated,
            "available_chars": snapshot.available_chars,
            "start_cursor": snapshot.start_cursor,
            "cursor": snapshot.cursor,
        }))
    }
}

async fn send_terminal_input_to_runtime(
    runtime: &McpRuntime,
    session_id: &str,
    data: String,
) -> Result<(), McpError> {
    if data.chars().count() > MAX_INPUT_CHARS {
        return Err(invalid_params(format!(
            "入力は{}文字以内で指定してください",
            MAX_INPUT_CHARS
        )));
    }

    let info = runtime
        .terminals
        .session_info(session_id)
        .await
        .ok_or_else(|| invalid_params("セッションが見つかりません"))?;

    if info.status != TerminalStatus::Connected {
        return Err(invalid_params("セッションは切断済みです"));
    }

    match info.protocol {
        TerminalProtocol::Ssh => ssh::write_data(&runtime.ssh, session_id, data).await,
        TerminalProtocol::Serial => serial::write_data(&runtime.serial, session_id, data).await,
        TerminalProtocol::Telnet => telnet::write_data(&runtime.telnet, session_id, data).await,
    }
    .map_err(internal_error)
}

async fn wait_for_terminal_output(
    terminals: &TerminalControlState,
    session_id: &str,
    start_cursor: usize,
    contains: Option<&str>,
    max_chars: usize,
    timeout_ms: u64,
) -> Result<Value, McpError> {
    let deadline = time::Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let output_changed = terminals.output_change_notified();
        tokio::pin!(output_changed);

        let snapshot = terminals
            .read_output_delta(session_id, start_cursor, max_chars)
            .await
            .map_err(invalid_params)?;
        let matched = match contains {
            Some(needle) => snapshot.output.contains(needle),
            None => !snapshot.output.is_empty(),
        };

        if matched {
            return Ok(json!({
                "session_id": snapshot.session_id,
                "matched": true,
                "timed_out": false,
                "output": snapshot.output,
                "truncated": snapshot.truncated,
                "available_chars": snapshot.available_chars,
                "start_cursor": snapshot.start_cursor,
                "cursor": snapshot.cursor,
            }));
        }

        let now = time::Instant::now();
        if now >= deadline {
            return Ok(json!({
                "session_id": snapshot.session_id,
                "matched": false,
                "timed_out": true,
                "output": snapshot.output,
                "truncated": snapshot.truncated,
                "available_chars": snapshot.available_chars,
                "start_cursor": snapshot.start_cursor,
                "cursor": snapshot.cursor,
            }));
        }

        let remaining = deadline - now;
        let _ = time::timeout(remaining, &mut output_changed).await;
    }
}

fn normalize_max_chars(max_chars: Option<usize>) -> usize {
    max_chars
        .unwrap_or(DEFAULT_READ_CHARS)
        .clamp(1, MAX_READ_CHARS)
}

fn normalize_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(DEFAULT_WAIT_TIMEOUT_MS)
        .clamp(1, MAX_WAIT_TIMEOUT_MS)
}

fn normalize_connect_dimension(value: Option<u32>, default_value: u32) -> u32 {
    value
        .unwrap_or(default_value)
        .clamp(1, MAX_CONNECT_DIMENSION)
}

fn normalize_profile_type(connection_type: &str) -> String {
    connection_type.trim().to_ascii_lowercase()
}

fn normalize_profile_host(profile: &SavedConnection) -> String {
    profile
        .host
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn normalize_profile_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_profile_encoding(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("shift-jis") => "shift-jis".into(),
        Some("euc-jp") => "euc-jp".into(),
        _ => "utf-8".into(),
    }
}

fn normalize_profile_terminal_mode(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("cisco_ios") => "cisco_ios".into(),
        _ => "general".into(),
    }
}

fn normalize_profile_auth_method(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("password") => Ok("password".into()),
        Some("public_key") => Ok("public_key".into()),
        Some(_) => Err("SSH認証方式が不正です".into()),
    }
}

fn ssh_credential_required(
    auth_method: &str,
    private_key_path: Option<&str>,
) -> Result<bool, String> {
    match auth_method {
        "password" => Ok(true),
        "public_key" => {
            let private_key_path = private_key_path
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "保存済みSSHプロファイルに秘密鍵ファイルが設定されていません".to_string()
                })?;
            ssh::private_key_requires_passphrase(private_key_path)
        }
        _ => Err("SSH認証方式が不正です".into()),
    }
}

fn available_serial_port_names(ports: &[serial::PortInfo]) -> String {
    if ports.is_empty() {
        "なし".into()
    } else {
        ports
            .iter()
            .map(|port| port.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn normalize_serial_data_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_DATA_BITS);
    match value {
        5 | 6 | 7 | 8 => Ok(value),
        _ => Err("data_bits は 5, 6, 7, 8 のいずれかを指定してください".into()),
    }
}

fn normalize_serial_stop_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_STOP_BITS);
    match value {
        1 | 2 => Ok(value),
        _ => Err("stop_bits は 1 または 2 を指定してください".into()),
    }
}

fn prepare_serial_console_connection(
    args: ConnectSerialConsoleArgs,
    available_ports: &[serial::PortInfo],
) -> Result<PreparedSerialConnection, String> {
    let port = args.port.trim().to_string();
    if port.is_empty() {
        return Err("port を指定してください".into());
    }
    if !available_ports
        .iter()
        .any(|available| available.name == port)
    {
        return Err(format!(
            "指定されたシリアルポートが見つかりません: {port}。利用可能: {}",
            available_serial_port_names(available_ports)
        ));
    }

    let baud_rate = args.baud_rate.unwrap_or(DEFAULT_SERIAL_BAUD_RATE);
    if baud_rate == 0 {
        return Err("baud_rate は 1 以上で指定してください".into());
    }

    let terminal_mode = args.terminal_mode.unwrap_or_default().as_str().to_string();
    let _cols = normalize_connect_dimension(args.cols, DEFAULT_CONNECT_COLS);
    let _rows = normalize_connect_dimension(args.rows, DEFAULT_CONNECT_ROWS);

    Ok(PreparedSerialConnection {
        port: port.clone(),
        config: serial::SerialConfig {
            baud_rate,
            data_bits: normalize_serial_data_bits(args.data_bits)?,
            parity: args.parity.unwrap_or_default().as_str().to_string(),
            stop_bits: normalize_serial_stop_bits(args.stop_bits)?,
            flow_control: args.flow_control.unwrap_or_default().as_str().to_string(),
        },
        target: port.clone(),
        title: port,
        encoding: "utf-8".into(),
        terminal_mode,
    })
}

fn list_connection_profiles_from_config(config: &AppConfig) -> Vec<McpConnectionProfile> {
    config
        .saved_connections
        .iter()
        .filter_map(|profile| {
            let connection_type = normalize_profile_type(&profile.connection_type);
            match connection_type.as_str() {
                "ssh" => Some(McpConnectionProfile {
                    id: profile.id.clone(),
                    connection_type,
                    host: normalize_profile_host(profile),
                    port: profile.port.unwrap_or(22),
                    username: normalize_profile_string(profile.username.as_deref()),
                    auth_method: Some(
                        profile
                            .auth_method
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .unwrap_or("password")
                            .to_string(),
                    ),
                    encoding: Some(normalize_profile_encoding(profile.encoding.as_deref())),
                    terminal_mode: Some(normalize_profile_terminal_mode(
                        profile.terminal_mode.as_deref(),
                    )),
                    private_key_configured: Some(
                        profile
                            .private_key_path
                            .as_deref()
                            .map(str::trim)
                            .is_some_and(|value| !value.is_empty()),
                    ),
                }),
                "telnet" => Some(McpConnectionProfile {
                    id: profile.id.clone(),
                    connection_type,
                    host: normalize_profile_host(profile),
                    port: profile.port.unwrap_or(23),
                    username: None,
                    auth_method: None,
                    encoding: Some(normalize_profile_encoding(profile.encoding.as_deref())),
                    terminal_mode: Some(normalize_profile_terminal_mode(
                        profile.terminal_mode.as_deref(),
                    )),
                    private_key_configured: None,
                }),
                _ => None,
            }
        })
        .collect()
}

fn prepare_saved_profile_connection(
    config: &AppConfig,
    args: ConnectSavedProfileArgs,
) -> Result<PreparedConnection, String> {
    let profile_id = args.profile_id.trim();
    if profile_id.is_empty() {
        return Err("profile_id を指定してください".into());
    }

    let profile = config
        .saved_connections
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "保存済みプロファイルが見つかりません".to_string())?;
    let connection_type = normalize_profile_type(&profile.connection_type);
    let host = normalize_profile_host(profile);
    if host.is_empty() {
        return Err("保存済みプロファイルにホストが設定されていません".into());
    }

    let encoding = normalize_profile_encoding(profile.encoding.as_deref());
    let terminal_mode = normalize_profile_terminal_mode(profile.terminal_mode.as_deref());
    let cols = normalize_connect_dimension(args.cols, DEFAULT_CONNECT_COLS);
    let rows = normalize_connect_dimension(args.rows, DEFAULT_CONNECT_ROWS);

    match connection_type.as_str() {
        "ssh" => {
            let username =
                normalize_profile_string(profile.username.as_deref()).ok_or_else(|| {
                    "保存済みSSHプロファイルにユーザー名が設定されていません".to_string()
                })?;
            let auth_method = normalize_profile_auth_method(profile.auth_method.as_deref())?;
            let private_key_path = normalize_profile_string(profile.private_key_path.as_deref());
            if auth_method == "public_key" && private_key_path.is_none() {
                return Err(
                    "保存済みSSHプロファイルに秘密鍵ファイルが設定されていません".to_string(),
                );
            }
            let port = profile.port.unwrap_or(22);
            Ok(PreparedConnection {
                kind: PreparedConnectionKind::Ssh {
                    host: host.clone(),
                    port,
                    username: username.clone(),
                    auth_method,
                    private_key_path,
                },
                profile_id: profile.id.clone(),
                connection_type,
                target: format!("{username}@{host}:{port}"),
                title: format!("{username}@{host}"),
                encoding,
                terminal_mode,
                cols,
                rows,
            })
        }
        "telnet" => {
            let port = profile.port.unwrap_or(23);
            Ok(PreparedConnection {
                kind: PreparedConnectionKind::Telnet {
                    host: host.clone(),
                    port,
                },
                profile_id: profile.id.clone(),
                connection_type,
                target: format!("{host}:{port}"),
                title: format!("{host}:{port}"),
                encoding,
                terminal_mode,
                cols,
                rows,
            })
        }
        _ => Err("MCP新規接続は保存済みSSH/Telnetプロファイルのみ対応しています".into()),
    }
}

#[cfg(not(test))]
async fn connect_prepared_profile(
    runtime: &McpRuntime,
    config: &AppConfig,
    prepared: PreparedConnection,
) -> Result<Value, McpError> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| internal_error("MCP接続に必要なアプリハンドルがありません"))?;

    let session_id = match &prepared.kind {
        PreparedConnectionKind::Ssh {
            host,
            port,
            username,
            auth_method,
            private_key_path,
        } => {
            ssh::verify_trusted_host_key(host, *port)
                .await
                .map_err(invalid_params)?;
            let credential = if ssh_credential_required(auth_method, private_key_path.as_deref())
                .map_err(invalid_params)?
            {
                let credentials = runtime
                    .credentials
                    .as_ref()
                    .ok_or_else(|| internal_error("MCP認証入力に必要な状態がありません"))?;
                Some(
                    credentials
                        .request_ssh_credential(
                            app,
                            McpCredentialRequestPayload {
                                request_id: String::new(),
                                profile_id: prepared.profile_id.clone(),
                                host: host.clone(),
                                port: *port,
                                username: username.clone(),
                                auth_method: auth_method.clone(),
                                target: prepared.target.clone(),
                                title: prepared.title.clone(),
                            },
                        )
                        .await
                        .map_err(invalid_params)?
                        .ok_or_else(|| invalid_params("MCP認証入力がキャンセルされました"))?,
                )
            } else {
                None
            };
            let (password, key_passphrase) = if auth_method == "password" {
                (credential.unwrap_or_default(), None)
            } else {
                (String::new(), credential)
            };

            ssh::connect(
                app,
                &runtime.ssh,
                &runtime.terminals,
                ssh::SshConnectOptions {
                    host: host.clone(),
                    port: *port,
                    username: username.clone(),
                    password,
                    auth_method: Some(auth_method.clone()),
                    private_key_path: private_key_path.clone(),
                    key_passphrase,
                    cols: prepared.cols,
                    rows: prepared.rows,
                },
            )
            .await
            .map_err(invalid_params)?
            .session_id
        }
        PreparedConnectionKind::Telnet { host, port } => telnet::connect(
            app,
            &runtime.telnet,
            &runtime.terminals,
            host.clone(),
            *port,
            prepared.cols,
            prepared.rows,
        )
        .await
        .map_err(invalid_params)?,
    };

    finish_created_session(
        runtime,
        config,
        session_id,
        prepared.connection_type,
        prepared.target,
        prepared.title,
        prepared.encoding,
        prepared.terminal_mode,
    )
    .await
}

#[cfg(not(test))]
async fn finish_created_session(
    runtime: &McpRuntime,
    config: &AppConfig,
    session_id: String,
    connection_type: String,
    target: String,
    title: String,
    encoding: String,
    terminal_mode: String,
) -> Result<Value, McpError> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| internal_error("MCP接続に必要なアプリハンドルがありません"))?;
    let auto_logging = if config.terminal.auto_session_log {
        match &runtime.logger {
            Some(logger_state) => logger::start_auto_log(
                logger_state,
                session_id.clone(),
                connection_type.clone(),
                target.clone(),
            )
            .await
            .map(|_| true)
            .unwrap_or_else(|error| {
                log::warn!("MCP auto log start failed for session {session_id}: {error}");
                false
            }),
            None => {
                log::warn!("MCP auto log start skipped because logger state is unavailable");
                false
            }
        }
    } else {
        false
    };

    let payload = McpConnectionCreatedPayload {
        session_id,
        connection_type,
        target,
        title,
        encoding,
        terminal_mode,
        auto_logging,
    };

    if let Err(error) = app.emit("terminal://created", &payload) {
        log::warn!("MCP terminal created event failed: {error}");
    }

    Ok(json!(payload))
}

#[cfg(not(test))]
async fn connect_prepared_serial_console(
    runtime: &McpRuntime,
    config: &AppConfig,
    prepared: PreparedSerialConnection,
) -> Result<Value, McpError> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| internal_error("MCP接続に必要なアプリハンドルがありません"))?;

    let session_id = serial::connect(
        app,
        &runtime.serial,
        &runtime.terminals,
        prepared.port.clone(),
        prepared.config,
    )
    .await
    .map_err(invalid_params)?;

    finish_created_session(
        runtime,
        config,
        session_id,
        "serial".into(),
        prepared.target,
        prepared.title,
        prepared.encoding,
        prepared.terminal_mode,
    )
    .await
}

#[cfg(test)]
async fn connect_prepared_profile(
    _runtime: &McpRuntime,
    _config: &AppConfig,
    _prepared: PreparedConnection,
) -> Result<Value, McpError> {
    Err(internal_error(
        "MCPプロファイル接続の実接続処理はユニットテストでは実行しません",
    ))
}

#[cfg(test)]
async fn connect_prepared_serial_console(
    _runtime: &McpRuntime,
    _config: &AppConfig,
    _prepared: PreparedSerialConnection,
) -> Result<Value, McpError> {
    Err(internal_error(
        "MCPシリアル接続の実接続処理はユニットテストでは実行しません",
    ))
}

#[derive(Clone)]
struct ExaTermMcpServer {
    service: McpTerminalService,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ExaTermMcpServer {
    fn new(runtime: McpRuntime) -> Self {
        Self {
            service: McpTerminalService::new(runtime),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "list_terminal_sessions",
        description = "List ExaTerm terminal sessions that were opened by the user."
    )]
    async fn list_terminal_sessions(&self) -> Result<CallToolResult, McpError> {
        self.service
            .list_terminal_sessions()
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "list_connection_profiles",
        description = "List saved SSH and Telnet connection profiles when MCP profile connections are enabled. Secrets and private key paths are not returned."
    )]
    async fn list_connection_profiles(&self) -> Result<CallToolResult, McpError> {
        self.service
            .list_connection_profiles()
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "connect_saved_profile",
        description = "Open a new ExaTerm SSH or Telnet session from a saved profile when MCP profile connections are enabled. SSH passwords and encrypted key passphrases are requested in the ExaTerm UI."
    )]
    async fn connect_saved_profile(
        &self,
        Parameters(args): Parameters<ConnectSavedProfileArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .connect_saved_profile(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "list_serial_ports",
        description = "List available Serial console ports when MCP profile connections are enabled."
    )]
    async fn list_serial_ports(&self) -> Result<CallToolResult, McpError> {
        self.service
            .list_serial_ports()
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "connect_serial_console",
        description = "Open a new ExaTerm Serial console session from explicit port and line settings when MCP profile connections are enabled."
    )]
    async fn connect_serial_console(
        &self,
        Parameters(args): Parameters<ConnectSerialConsoleArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .connect_serial_console(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "read_terminal_output",
        description = "Read recent output from an ExaTerm terminal session."
    )]
    async fn read_terminal_output(
        &self,
        Parameters(args): Parameters<ReadTerminalOutputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .read_terminal_output(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "read_terminal_output_delta",
        description = "Read output written after a previously returned ExaTerm terminal cursor."
    )]
    async fn read_terminal_output_delta(
        &self,
        Parameters(args): Parameters<ReadTerminalOutputDeltaArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .read_terminal_output_delta(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "wait_terminal_output",
        description = "Wait until an ExaTerm terminal session produces new output or a target string appears."
    )]
    async fn wait_terminal_output(
        &self,
        Parameters(args): Parameters<WaitTerminalOutputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .wait_terminal_output(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "send_terminal_input",
        description = "Send text to an existing connected ExaTerm terminal session."
    )]
    async fn send_terminal_input(
        &self,
        Parameters(args): Parameters<SendTerminalInputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .send_terminal_input(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "run_terminal_command",
        description = "Send a command to an existing connected ExaTerm terminal session, wait for output, and return the output delta."
    )]
    async fn run_terminal_command(
        &self,
        Parameters(args): Parameters<RunTerminalCommandArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .run_terminal_command(args)
            .await
            .and_then(structured_tool_result)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ExaTermMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("exaterm", env!("CARGO_PKG_VERSION")))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct McpConnectionProfile {
    id: String,
    connection_type: String,
    host: String,
    port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key_configured: Option<bool>,
}

#[cfg(not(test))]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct McpConnectionCreatedPayload {
    session_id: String,
    connection_type: String,
    target: String,
    title: String,
    encoding: String,
    terminal_mode: String,
    auto_logging: bool,
}

#[cfg(not(test))]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct McpCredentialRequestPayload {
    request_id: String,
    profile_id: String,
    host: String,
    port: u16,
    username: String,
    auth_method: String,
    target: String,
    title: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ConnectSavedProfileArgs {
    /// Saved profile ID from list_connection_profiles.
    profile_id: String,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    rows: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum McpSerialParity {
    #[default]
    None,
    Odd,
    Even,
}

impl McpSerialParity {
    fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Odd => "odd",
            Self::Even => "even",
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum McpSerialFlowControl {
    #[default]
    None,
    Software,
    Hardware,
}

impl McpSerialFlowControl {
    fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Software => "software",
            Self::Hardware => "hardware",
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum McpTerminalMode {
    #[default]
    General,
    CiscoIos,
}

impl McpTerminalMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::CiscoIos => "cisco_ios",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ConnectSerialConsoleArgs {
    /// Serial port name, for example COM3.
    port: String,
    /// Baud rate. Defaults to 9600.
    #[schemars(range(min = 1))]
    baud_rate: Option<u32>,
    /// Data bits. Supported values are 5, 6, 7, and 8. Defaults to 8.
    #[schemars(range(min = 5, max = 8))]
    data_bits: Option<u8>,
    /// Parity. Defaults to none.
    parity: Option<McpSerialParity>,
    /// Stop bits. Supported values are 1 and 2. Defaults to 1.
    #[schemars(range(min = 1, max = 2))]
    stop_bits: Option<u8>,
    /// Flow control. Defaults to none.
    flow_control: Option<McpSerialFlowControl>,
    /// Initial terminal mode. Defaults to general.
    terminal_mode: Option<McpTerminalMode>,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    rows: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedConnection {
    kind: PreparedConnectionKind,
    profile_id: String,
    connection_type: String,
    target: String,
    title: String,
    encoding: String,
    terminal_mode: String,
    cols: u32,
    rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparedConnectionKind {
    Ssh {
        host: String,
        port: u16,
        username: String,
        auth_method: String,
        private_key_path: Option<String>,
    },
    Telnet {
        host: String,
        port: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedSerialConnection {
    port: String,
    config: serial::SerialConfig,
    target: String,
    title: String,
    encoding: String,
    terminal_mode: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadTerminalOutputArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadTerminalOutputDeltaArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Cursor returned by read_terminal_output, read_terminal_output_delta, wait_terminal_output, or run_terminal_command.
    cursor: usize,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WaitTerminalOutputArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Cursor to wait from. When omitted, waits from the current output cursor.
    cursor: Option<usize>,
    /// Optional substring to wait for in the output delta.
    contains: Option<String>,
    /// Maximum wait time in milliseconds.
    #[schemars(range(min = 1, max = MAX_WAIT_TIMEOUT_MS))]
    timeout_ms: Option<u64>,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SendTerminalInputArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Text to send to the terminal. Include newline characters when needed.
    data: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RunTerminalCommandArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Command text to send to the terminal.
    command: String,
    /// Append a newline after the command. Defaults to true.
    append_newline: Option<bool>,
    /// Optional substring to wait for in the output delta after sending.
    wait_contains: Option<String>,
    /// Maximum wait time in milliseconds.
    #[schemars(range(min = 1, max = MAX_WAIT_TIMEOUT_MS))]
    timeout_ms: Option<u64>,
    /// Additional quiet period after a match before the final output delta is returned.
    #[schemars(range(min = 0, max = MAX_SETTLE_MS))]
    settle_ms: Option<u64>,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

type ExaTermMcpHttpService = StreamableHttpService<ExaTermMcpServer, LocalSessionManager>;

pub fn spawn_mcp_server(runtime: McpRuntime) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_mcp_server(runtime).await {
            log::error!("MCP server stopped: {}", error);
        }
    });
}

async fn run_mcp_server(runtime: McpRuntime) -> Result<(), String> {
    let address = format!("{}:{}", runtime.config.host, runtime.config.port);
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|error| format!("MCP bind error on {}: {}", address, error))?;

    let mcp_runtime = runtime.clone();
    let service = StreamableHttpService::new(
        move || Ok(ExaTermMcpServer::new(mcp_runtime.clone())),
        Arc::new(LocalSessionManager::default()),
        mcp_server_config(&runtime.config.host),
    );
    let app = Router::new()
        .route("/mcp", any(mcp_http_handler))
        .with_state(service);

    log::info!("MCP server listening on http://{}/mcp", address);
    axum::serve(listener, app)
        .await
        .map_err(|error| format!("MCP serve error on {}: {}", address, error))
}

fn mcp_server_config(configured_host: &str) -> StreamableHttpServerConfig {
    let mut allowed_hosts = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let configured_host = configured_host.trim();
    if !configured_host.is_empty()
        && configured_host != "0.0.0.0"
        && configured_host != "::"
        && !allowed_hosts.iter().any(|host| host == configured_host)
    {
        allowed_hosts.push(configured_host.to_string());
    }

    StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(allowed_hosts)
}

async fn mcp_http_handler(
    State(service): State<ExaTermMcpHttpService>,
    request: Request<Body>,
) -> Response {
    if !origin_is_allowed(request.headers()) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    service.handle(request).await.map(Body::new)
}

fn structured_tool_result(value: Value) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| internal_error(format!("Serialize MCP tool result failed: {error}")))?;
    let mut result = CallToolResult::structured(value);
    result.content = vec![Content::text(text)];
    Ok(result)
}

fn invalid_params(message: impl Into<String>) -> McpError {
    McpError::invalid_params(message.into(), None)
}

fn internal_error(message: impl Into<String>) -> McpError {
    McpError::internal_error(message.into(), None)
}

fn origin_is_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin") else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    if origin == "null" {
        return true;
    }

    let Some(origin_without_scheme) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = origin_without_scheme
        .split('/')
        .next()
        .unwrap_or(origin_without_scheme);
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        authority.split(':').next().unwrap_or(authority)
    };

    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_control::TerminalProtocol;
    use axum::body::to_bytes;

    fn test_runtime() -> McpRuntime {
        McpRuntime {
            config: McpConfig::default(),
            terminals: TerminalControlState::new(),
            ssh: SshState::new(),
            serial: SerialState::new(),
            telnet: TelnetState::new(),
        }
    }

    fn test_http_service(runtime: McpRuntime) -> ExaTermMcpHttpService {
        let mcp_runtime = runtime.clone();
        StreamableHttpService::new(
            move || Ok(ExaTermMcpServer::new(mcp_runtime.clone())),
            Arc::new(LocalSessionManager::default()),
            mcp_server_config(&runtime.config.host),
        )
    }

    async fn post_mcp(service: ExaTermMcpHttpService, body: Value) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("host", "127.0.0.1:8765")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = service.handle(request).await.map(Body::new);
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value = serde_json::from_slice(&body).unwrap_or_else(|error| {
            panic!(
                "MCP response was not JSON: status={status}, error={error}, body={:?}",
                String::from_utf8_lossy(&body)
            )
        });
        (status, value)
    }

    #[test]
    fn rejects_non_local_origins() {
        let mut headers = HeaderMap::new();
        headers.insert("origin", "https://example.com".parse().unwrap());
        assert!(!origin_is_allowed(&headers));

        headers.insert("origin", "http://127.0.0.1:8765".parse().unwrap());
        assert!(origin_is_allowed(&headers));

        headers.insert("origin", "http://localhost.evil.test".parse().unwrap());
        assert!(!origin_is_allowed(&headers));

        headers.insert("origin", "http://[::1]:8765".parse().unwrap());
        assert!(origin_is_allowed(&headers));
    }

    #[test]
    fn mcp_server_config_uses_stateless_json_responses() {
        let config = mcp_server_config("127.0.0.1");
        assert!(!config.stateful_mode);
        assert!(config.json_response);
    }

    #[tokio::test]
    async fn connection_tools_require_connect_enabled() {
        let service = McpTerminalService::new(test_runtime());

        let error = service.list_connection_profiles().await.unwrap_err();
        assert!(error.message.contains("connect_enabled"));

        let error = service
            .connect_saved_profile(ConnectSavedProfileArgs {
                profile_id: "dev".into(),
                cols: None,
                rows: None,
            })
            .await
            .unwrap_err();
        assert!(error.message.contains("connect_enabled"));

        let error = service.list_serial_ports().await.unwrap_err();
        assert!(error.message.contains("connect_enabled"));

        let error = service
            .connect_serial_console(ConnectSerialConsoleArgs {
                port: "COM1".into(),
                baud_rate: None,
                data_bits: None,
                parity: None,
                stop_bits: None,
                flow_control: None,
                terminal_mode: None,
                cols: None,
                rows: None,
            })
            .await
            .unwrap_err();
        assert!(error.message.contains("connect_enabled"));
    }

    #[test]
    fn list_connection_profiles_returns_only_ssh_telnet_without_secret_paths() {
        let mut config = AppConfig::default();
        config.saved_connections = vec![
            SavedConnection {
                id: "dev".into(),
                connection_type: "ssh".into(),
                host: Some("192.0.2.10".into()),
                port: Some(2222),
                username: Some("admin".into()),
                auth_method: Some("public_key".into()),
                private_key_path: Some("C:\\Users\\me\\.ssh\\id_ed25519".into()),
                encoding: Some("shift-jis".into()),
                terminal_mode: Some("cisco_ios".into()),
            },
            SavedConnection {
                id: "legacy".into(),
                connection_type: "telnet".into(),
                host: Some("192.0.2.20".into()),
                port: None,
                encoding: Some("euc-jp".into()),
                ..SavedConnection::default()
            },
            SavedConnection {
                id: "console".into(),
                connection_type: "serial".into(),
                ..SavedConnection::default()
            },
        ];

        let profiles = list_connection_profiles_from_config(&config);

        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].id, "dev");
        assert_eq!(profiles[0].private_key_configured, Some(true));
        assert_eq!(profiles[1].id, "legacy");
        assert_eq!(profiles[1].port, 23);
        let serialized = serde_json::to_string(&profiles).unwrap();
        assert!(!serialized.contains("private_key_path"));
        assert!(!serialized.contains("id_ed25519"));
    }

    #[test]
    fn prepare_saved_profile_rejects_missing_and_unsupported_profiles() {
        let mut config = AppConfig::default();
        config.saved_connections = vec![SavedConnection {
            id: "console".into(),
            connection_type: "serial".into(),
            host: Some("COM1".into()),
            ..SavedConnection::default()
        }];

        let missing = prepare_saved_profile_connection(
            &config,
            ConnectSavedProfileArgs {
                profile_id: "missing".into(),
                cols: None,
                rows: None,
            },
        )
        .unwrap_err();
        assert!(missing.contains("見つかりません"));

        let unsupported = prepare_saved_profile_connection(
            &config,
            ConnectSavedProfileArgs {
                profile_id: "console".into(),
                cols: None,
                rows: None,
            },
        )
        .unwrap_err();
        assert!(unsupported.contains("SSH/Telnet"));
    }

    #[test]
    fn prepare_saved_profile_rejects_incomplete_ssh_profiles() {
        let mut config = AppConfig::default();
        config.saved_connections = vec![
            SavedConnection {
                id: "missing-user".into(),
                connection_type: "ssh".into(),
                host: Some("192.0.2.10".into()),
                auth_method: Some("password".into()),
                ..SavedConnection::default()
            },
            SavedConnection {
                id: "key".into(),
                connection_type: "ssh".into(),
                host: Some("192.0.2.10".into()),
                username: Some("admin".into()),
                auth_method: Some("public_key".into()),
                private_key_path: Some("  ".into()),
                ..SavedConnection::default()
            },
        ];

        let user_error = prepare_saved_profile_connection(
            &config,
            ConnectSavedProfileArgs {
                profile_id: "missing-user".into(),
                cols: None,
                rows: None,
            },
        )
        .unwrap_err();
        assert!(user_error.contains("ユーザー名"));

        let key_error = prepare_saved_profile_connection(
            &config,
            ConnectSavedProfileArgs {
                profile_id: "key".into(),
                cols: None,
                rows: None,
            },
        )
        .unwrap_err();
        assert!(key_error.contains("秘密鍵"));
    }

    #[test]
    fn prepare_saved_profile_builds_connection_metadata() {
        let mut config = AppConfig::default();
        config.saved_connections = vec![SavedConnection {
            id: "dev".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            port: Some(2222),
            username: Some("admin".into()),
            auth_method: Some("password".into()),
            encoding: Some("shift-jis".into()),
            terminal_mode: Some("cisco_ios".into()),
            ..SavedConnection::default()
        }];

        let prepared = prepare_saved_profile_connection(
            &config,
            ConnectSavedProfileArgs {
                profile_id: "dev".into(),
                cols: Some(80),
                rows: Some(24),
            },
        )
        .unwrap();

        assert_eq!(prepared.connection_type, "ssh");
        assert_eq!(prepared.profile_id, "dev");
        assert_eq!(prepared.target, "admin@192.0.2.10:2222");
        assert_eq!(prepared.title, "admin@192.0.2.10");
        assert_eq!(prepared.encoding, "shift-jis");
        assert_eq!(prepared.terminal_mode, "cisco_ios");
        assert_eq!(prepared.cols, 80);
        assert_eq!(prepared.rows, 24);
    }

    #[test]
    fn ssh_credential_required_keeps_password_prompt_and_rejects_missing_key() {
        assert!(ssh_credential_required("password", None).unwrap());

        let error = ssh_credential_required("public_key", None).unwrap_err();
        assert!(error.contains("秘密鍵"));
    }

    #[test]
    fn prepare_serial_console_uses_defaults_and_line_settings() {
        let ports = vec![serial::PortInfo {
            name: "COM3".into(),
            port_type: "USB".into(),
        }];

        let prepared = prepare_serial_console_connection(
            ConnectSerialConsoleArgs {
                port: " COM3 ".into(),
                baud_rate: Some(115_200),
                data_bits: Some(7),
                parity: Some(McpSerialParity::Even),
                stop_bits: Some(2),
                flow_control: Some(McpSerialFlowControl::Hardware),
                terminal_mode: Some(McpTerminalMode::CiscoIos),
                cols: Some(80),
                rows: Some(24),
            },
            &ports,
        )
        .unwrap();

        assert_eq!(prepared.port, "COM3");
        assert_eq!(prepared.config.baud_rate, 115_200);
        assert_eq!(prepared.config.data_bits, 7);
        assert_eq!(prepared.config.parity, "even");
        assert_eq!(prepared.config.stop_bits, 2);
        assert_eq!(prepared.config.flow_control, "hardware");
        assert_eq!(prepared.target, "COM3");
        assert_eq!(prepared.title, "COM3");
        assert_eq!(prepared.encoding, "utf-8");
        assert_eq!(prepared.terminal_mode, "cisco_ios");

        let defaulted = prepare_serial_console_connection(
            ConnectSerialConsoleArgs {
                port: "COM3".into(),
                baud_rate: None,
                data_bits: None,
                parity: None,
                stop_bits: None,
                flow_control: None,
                terminal_mode: None,
                cols: None,
                rows: None,
            },
            &ports,
        )
        .unwrap();
        assert_eq!(defaulted.config, serial::SerialConfig::default());
        assert_eq!(defaulted.terminal_mode, "general");
    }

    #[test]
    fn prepare_serial_console_rejects_missing_port_and_invalid_settings() {
        let ports = vec![serial::PortInfo {
            name: "COM5".into(),
            port_type: "USB".into(),
        }];

        let missing = prepare_serial_console_connection(
            ConnectSerialConsoleArgs {
                port: "COM4".into(),
                baud_rate: None,
                data_bits: None,
                parity: None,
                stop_bits: None,
                flow_control: None,
                terminal_mode: None,
                cols: None,
                rows: None,
            },
            &ports,
        )
        .unwrap_err();
        assert!(missing.contains("COM4"));
        assert!(missing.contains("COM5"));

        let invalid_data_bits = prepare_serial_console_connection(
            ConnectSerialConsoleArgs {
                port: "COM5".into(),
                baud_rate: None,
                data_bits: Some(9),
                parity: None,
                stop_bits: None,
                flow_control: None,
                terminal_mode: None,
                cols: None,
                rows: None,
            },
            &ports,
        )
        .unwrap_err();
        assert!(invalid_data_bits.contains("data_bits"));

        let invalid_stop_bits = prepare_serial_console_connection(
            ConnectSerialConsoleArgs {
                port: "COM5".into(),
                baud_rate: None,
                data_bits: None,
                parity: None,
                stop_bits: Some(3),
                flow_control: None,
                terminal_mode: None,
                cols: None,
                rows: None,
            },
            &ports,
        )
        .unwrap_err();
        assert!(invalid_stop_bits.contains("stop_bits"));
    }

    #[tokio::test]
    async fn service_lists_terminal_sessions() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
            .await;
        let service = McpTerminalService::new(runtime);

        let result = service.list_terminal_sessions().await.unwrap();
        assert_eq!(result["sessions"][0]["session_id"], "s1");
        assert_eq!(result["sessions"][0]["protocol"], "ssh");
        assert_eq!(result["sessions"][0]["status"], "connected");
    }

    #[tokio::test]
    async fn service_reads_terminal_output_with_multibyte_tail() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
            .await;
        runtime
            .terminals
            .append_output("s1", "こんにちは世界".as_bytes())
            .await;
        let service = McpTerminalService::new(runtime);

        let result = service
            .read_terminal_output(ReadTerminalOutputArgs {
                session_id: "s1".into(),
                max_chars: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(result["session_id"], "s1");
        assert_eq!(result["output"], "世界");
        assert_eq!(result["truncated"], true);
        assert_eq!(result["start_cursor"], 5);
        assert_eq!(result["cursor"], 7);
    }

    #[tokio::test]
    async fn service_reads_terminal_output_delta() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
            .await;
        runtime
            .terminals
            .append_output("s1", "abcこんにちは".as_bytes())
            .await;
        let service = McpTerminalService::new(runtime);

        let result = service
            .read_terminal_output_delta(ReadTerminalOutputDeltaArgs {
                session_id: "s1".into(),
                cursor: 3,
                max_chars: Some(100),
            })
            .await
            .unwrap();

        assert_eq!(result["output"], "こんにちは");
        assert_eq!(result["start_cursor"], 3);
        assert_eq!(result["cursor"], 8);
    }

    #[tokio::test]
    async fn service_waits_for_matching_terminal_output() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
            .await;
        let terminals = runtime.terminals.clone();
        tokio::spawn(async move {
            time::sleep(Duration::from_millis(10)).await;
            terminals.append_output("s1", b"router#").await;
        });
        let service = McpTerminalService::new(runtime);

        let result = service
            .wait_terminal_output(WaitTerminalOutputArgs {
                session_id: "s1".into(),
                cursor: Some(0),
                contains: Some("router#".into()),
                timeout_ms: Some(500),
                max_chars: Some(100),
            })
            .await
            .unwrap();

        assert_eq!(result["matched"], true);
        assert_eq!(result["timed_out"], false);
        assert_eq!(result["output"], "router#");
    }

    #[tokio::test]
    async fn service_wait_timeout_returns_latest_delta() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
            .await;
        runtime.terminals.append_output("s1", b"partial").await;
        let service = McpTerminalService::new(runtime);

        let result = service
            .wait_terminal_output(WaitTerminalOutputArgs {
                session_id: "s1".into(),
                cursor: Some(0),
                contains: Some("missing".into()),
                timeout_ms: Some(1),
                max_chars: Some(100),
            })
            .await
            .unwrap();

        assert_eq!(result["matched"], false);
        assert_eq!(result["timed_out"], true);
        assert_eq!(result["output"], "partial");
    }

    #[tokio::test]
    async fn service_rejects_send_to_disconnected_session() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Telnet, "host:23".into())
            .await;
        runtime.terminals.mark_disconnected("s1").await;
        let service = McpTerminalService::new(runtime);

        let error = service
            .send_terminal_input(SendTerminalInputArgs {
                session_id: "s1".into(),
                data: "show version\n".into(),
            })
            .await
            .unwrap_err();
        assert!(error.message.contains("切断済み"));
    }

    #[tokio::test]
    async fn service_rejects_empty_run_terminal_command() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
            .await;
        let service = McpTerminalService::new(runtime);

        let error = service
            .run_terminal_command(RunTerminalCommandArgs {
                session_id: "s1".into(),
                command: "   ".into(),
                append_newline: None,
                wait_contains: None,
                timeout_ms: None,
                settle_ms: None,
                max_chars: None,
            })
            .await
            .unwrap_err();

        assert!(error.message.contains("空"));
    }

    #[tokio::test]
    async fn rmcp_http_service_handles_initialize_and_tools_list() {
        let service = test_http_service(test_runtime());

        let (status, initialize) = post_mcp(
            service.clone(),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "test-client",
                        "version": "0.0.0"
                    }
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(initialize["result"]["serverInfo"]["name"], "exaterm");

        let (status, tools) = post_mcp(
            service,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let names = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "connect_saved_profile",
                "connect_serial_console",
                "list_connection_profiles",
                "list_serial_ports",
                "list_terminal_sessions",
                "read_terminal_output",
                "read_terminal_output_delta",
                "run_terminal_command",
                "send_terminal_input",
                "wait_terminal_output",
            ]
        );

        let read_tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "read_terminal_output")
            .unwrap();
        let max_chars_schema = &read_tool["inputSchema"]["properties"]["max_chars"];
        assert_eq!(max_chars_schema["minimum"], 1);
        assert_eq!(max_chars_schema["maximum"], MAX_READ_CHARS);

        let wait_tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wait_terminal_output")
            .unwrap();
        let timeout_schema = &wait_tool["inputSchema"]["properties"]["timeout_ms"];
        assert_eq!(timeout_schema["minimum"], 1);
        assert_eq!(timeout_schema["maximum"], MAX_WAIT_TIMEOUT_MS);

        let connect_tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "connect_saved_profile")
            .unwrap();
        let connect_properties = connect_tool["inputSchema"]["properties"]
            .as_object()
            .unwrap();
        assert!(connect_properties.contains_key("profile_id"));
        assert!(!connect_properties.contains_key("credential"));

        let serial_connect_tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "connect_serial_console")
            .unwrap();
        let serial_properties = serial_connect_tool["inputSchema"]["properties"]
            .as_object()
            .unwrap();
        for property in [
            "port",
            "baud_rate",
            "data_bits",
            "parity",
            "stop_bits",
            "flow_control",
            "terminal_mode",
            "cols",
            "rows",
        ] {
            assert!(serial_properties.contains_key(property));
        }
        assert_eq!(serial_properties["cols"]["maximum"], MAX_CONNECT_DIMENSION);
        assert_eq!(serial_properties["rows"]["maximum"], MAX_CONNECT_DIMENSION);
    }
}
