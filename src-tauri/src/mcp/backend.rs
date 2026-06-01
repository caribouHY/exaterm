use std::time::Duration;

use async_trait::async_trait;
use rmcp::{
    model::{CallToolResult, Content},
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(not(test))]
use tauri::AppHandle;
use tokio::time;
#[cfg(not(test))]
use uuid::Uuid;

use crate::config::{self, AppConfig, McpConfig, SavedConnection};
use crate::logger::{self, LoggerState};
#[cfg(not(test))]
use crate::mcp::control::McpCredentialState;
use crate::mcp::control::McpLogControlState;
use crate::serial::{self, SerialState};
use crate::ssh::{self, SshState};
use crate::telnet::{self, TelnetState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};
use crate::workspace::WorkspaceState;
#[cfg(not(test))]
use crate::workspace::{emit_workspace_updated, WorkspaceTabRegisterInput};

pub(super) const DEFAULT_READ_CHARS: usize = 2_000;
pub(super) const MAX_READ_CHARS: usize = 20_000;
const MAX_INPUT_CHARS: usize = 20_000;
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 10_000;
pub(super) const MAX_WAIT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_SETTLE_MS: u64 = 250;
const MAX_SETTLE_MS: u64 = 5_000;
const DEFAULT_CONNECT_COLS: u32 = 120;
const DEFAULT_CONNECT_ROWS: u32 = 30;
pub(super) const MAX_CONNECT_DIMENSION: u32 = 1_000;
const DEFAULT_SERIAL_BAUD_RATE: u32 = 9_600;
const DEFAULT_SERIAL_DATA_BITS: u8 = 8;
const DEFAULT_SERIAL_STOP_BITS: u8 = 1;

#[derive(Clone)]
pub struct McpRuntime {
    pub config: McpConfig,
    #[cfg(not(test))]
    pub app: Option<AppHandle>,
    pub terminals: TerminalControlState,
    #[cfg_attr(test, allow(dead_code))]
    pub workspace: WorkspaceState,
    pub ssh: SshState,
    pub serial: SerialState,
    pub telnet: TelnetState,
    pub logger: Option<LoggerState>,
    #[cfg_attr(test, allow(dead_code))]
    pub log_control: Option<McpLogControlState>,
    #[cfg(not(test))]
    pub credentials: Option<McpCredentialState>,
}

#[derive(Clone)]
pub(super) struct McpTerminalService {
    pub(super) runtime: McpRuntime,
}

impl McpTerminalService {
    pub(super) fn new(runtime: McpRuntime) -> Self {
        Self { runtime }
    }

    pub(super) async fn list_terminal_sessions(&self) -> Result<Value, McpError> {
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

    pub(super) async fn list_connection_profiles(&self) -> Result<Value, McpError> {
        self.ensure_connect_enabled()?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        Ok(json!({
            "profiles": list_connection_profiles_from_config(&config),
        }))
    }

    pub(super) async fn connect_saved_profile(
        &self,
        args: ConnectSavedProfileArgs,
    ) -> Result<Value, McpError> {
        self.ensure_connect_enabled()?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        let prepared = prepare_saved_profile_connection(&config, args).map_err(invalid_params)?;
        connect_prepared_profile(&self.runtime, &config, prepared).await
    }

    pub(super) async fn list_serial_ports(&self) -> Result<Value, McpError> {
        self.ensure_connect_enabled()?;
        let ports = serial::serial_list_ports().map_err(internal_error)?;
        Ok(json!({
            "ports": ports,
        }))
    }

    pub(super) async fn connect_serial_console(
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

    pub(super) async fn read_terminal_output(
        &self,
        args: ReadTerminalOutputArgs,
    ) -> Result<Value, McpError> {
        let snapshot = self
            .runtime
            .terminals
            .read_output(&args.session_id, normalize_max_chars(args.max_chars))
            .await
            .map_err(invalid_params)?;

        Ok(json!(snapshot))
    }

    pub(super) async fn read_terminal_output_delta(
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

    pub(super) async fn wait_terminal_output(
        &self,
        args: WaitTerminalOutputArgs,
    ) -> Result<Value, McpError> {
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

    pub(super) async fn send_terminal_input(
        &self,
        args: SendTerminalInputArgs,
    ) -> Result<Value, McpError> {
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, args.data).await?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
        }))
    }

    pub(super) async fn start_terminal_log(
        &self,
        args: StartTerminalLogArgs,
    ) -> Result<Value, McpError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| invalid_params("セッションが見つかりません"))?;

        if info.status != TerminalStatus::Connected {
            return Err(invalid_params("セッションは切断済みです"));
        }

        let logger_state = self
            .runtime
            .logger
            .as_ref()
            .ok_or_else(|| internal_error("MCPログ開始に必要なロガー状態がありません"))?;
        if let Some(session) = logger::manual_log_session(logger_state, &args.session_id).await {
            return Ok(json!({
                "session_id": args.session_id,
                "started": false,
                "already_active": true,
                "file_path": session.file_path,
                "log_mode": "manual",
            }));
        }

        let file_path = request_manual_log_start(&self.runtime, &info)
            .await
            .map_err(internal_error)?;

        Ok(json!({
            "session_id": args.session_id,
            "started": true,
            "already_active": false,
            "file_path": file_path,
            "log_mode": "manual",
        }))
    }

    pub(super) async fn stop_terminal_log(
        &self,
        args: StopTerminalLogArgs,
    ) -> Result<Value, McpError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| invalid_params("セッションが見つかりません"))?;

        let logger_state = self
            .runtime
            .logger
            .as_ref()
            .ok_or_else(|| internal_error("MCPログ停止に必要なロガー状態がありません"))?;
        if logger::manual_log_session(logger_state, &args.session_id)
            .await
            .is_none()
        {
            return Ok(json!({
                "session_id": args.session_id,
                "stopped": false,
                "already_inactive": true,
            }));
        }

        request_manual_log_stop(&self.runtime, &info)
            .await
            .map_err(internal_error)?;

        Ok(json!({
            "session_id": args.session_id,
            "stopped": true,
            "already_inactive": false,
        }))
    }

    pub(super) async fn run_terminal_command(
        &self,
        args: RunTerminalCommandArgs,
    ) -> Result<Value, McpError> {
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

#[async_trait]
pub trait McpBackend: Send + Sync {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError>;
}

#[derive(Clone)]
pub struct InProcessMcpBackend {
    service: McpTerminalService,
}

impl InProcessMcpBackend {
    pub fn new(runtime: McpRuntime) -> Self {
        Self {
            service: McpTerminalService::new(runtime),
        }
    }
}

#[async_trait]
impl McpBackend for InProcessMcpBackend {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        match name {
            "list_terminal_sessions" => self.service.list_terminal_sessions().await,
            "list_connection_profiles" => self.service.list_connection_profiles().await,
            "connect_saved_profile" => {
                self.service
                    .connect_saved_profile(parse_tool_args(args)?)
                    .await
            }
            "list_serial_ports" => self.service.list_serial_ports().await,
            "connect_serial_console" => {
                self.service
                    .connect_serial_console(parse_tool_args(args)?)
                    .await
            }
            "read_terminal_output" => {
                self.service
                    .read_terminal_output(parse_tool_args(args)?)
                    .await
            }
            "read_terminal_output_delta" => {
                self.service
                    .read_terminal_output_delta(parse_tool_args(args)?)
                    .await
            }
            "wait_terminal_output" => {
                self.service
                    .wait_terminal_output(parse_tool_args(args)?)
                    .await
            }
            "send_terminal_input" => {
                self.service
                    .send_terminal_input(parse_tool_args(args)?)
                    .await
            }
            "start_terminal_log" => {
                self.service
                    .start_terminal_log(parse_tool_args(args)?)
                    .await
            }
            "stop_terminal_log" => self.service.stop_terminal_log(parse_tool_args(args)?).await,
            "run_terminal_command" => {
                self.service
                    .run_terminal_command(parse_tool_args(args)?)
                    .await
            }
            _ => Err(invalid_params(format!("Unknown MCP tool: {name}"))),
        }
    }
}

fn parse_tool_args<T>(args: Value) -> Result<T, McpError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(args)
        .map_err(|error| invalid_params(format!("MCP tool argument parse failed: {error}")))
}

#[cfg(not(test))]
async fn request_manual_log_start(
    runtime: &McpRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<String, String> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| "MCPログ開始に必要なアプリハンドルがありません".to_string())?;
    let log_control = runtime
        .log_control
        .as_ref()
        .ok_or_else(|| "MCPログ開始に必要なログ制御状態がありません".to_string())?;
    let ack = log_control
        .request(
            app,
            "mcp://log-start-request",
            McpLogControlRequestPayload {
                request_id: Uuid::new_v4().to_string(),
                session_id: info.session_id.clone(),
                connection_type: terminal_protocol_log_type(info.protocol).into(),
                target: info.target.clone(),
            },
        )
        .await?;
    ack.file_path
        .ok_or_else(|| "MCPログ開始応答にログファイルパスがありません".to_string())
}

#[cfg(test)]
async fn request_manual_log_start(
    runtime: &McpRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<String, String> {
    let logger_state = runtime
        .logger
        .as_ref()
        .ok_or_else(|| "MCPログ開始に必要なロガー状態がありません".to_string())?;
    logger::start_manual_log(
        logger_state,
        info.session_id.clone(),
        terminal_protocol_log_type(info.protocol).into(),
        info.target.clone(),
        None,
        None,
    )
    .await
}

#[cfg(not(test))]
async fn request_manual_log_stop(
    runtime: &McpRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<(), String> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| "MCPログ停止に必要なアプリハンドルがありません".to_string())?;
    let log_control = runtime
        .log_control
        .as_ref()
        .ok_or_else(|| "MCPログ停止に必要なログ制御状態がありません".to_string())?;
    log_control
        .request(
            app,
            "mcp://log-stop-request",
            McpLogControlRequestPayload {
                request_id: Uuid::new_v4().to_string(),
                session_id: info.session_id.clone(),
                connection_type: terminal_protocol_log_type(info.protocol).into(),
                target: info.target.clone(),
            },
        )
        .await
        .map(|_| ())
}

#[cfg(test)]
async fn request_manual_log_stop(
    runtime: &McpRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<(), String> {
    let logger_state = runtime
        .logger
        .as_ref()
        .ok_or_else(|| "MCPログ停止に必要なロガー状態がありません".to_string())?;
    logger::stop_manual_log(logger_state, &info.session_id).await
}

fn terminal_protocol_log_type(protocol: TerminalProtocol) -> &'static str {
    match protocol {
        TerminalProtocol::Ssh => "ssh",
        TerminalProtocol::Serial => "serial",
        TerminalProtocol::Telnet => "telnet",
    }
}

#[cfg(not(test))]
fn terminal_protocol_from_log_type(value: &str) -> Result<TerminalProtocol, String> {
    match value {
        "ssh" => Ok(TerminalProtocol::Ssh),
        "serial" => Ok(TerminalProtocol::Serial),
        "telnet" => Ok(TerminalProtocol::Telnet),
        _ => Err(format!("不明な接続種別: {value}")),
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

pub(super) fn normalize_max_chars(max_chars: Option<usize>) -> usize {
    max_chars
        .unwrap_or(DEFAULT_READ_CHARS)
        .clamp(1, MAX_READ_CHARS)
}

pub(super) fn normalize_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(DEFAULT_WAIT_TIMEOUT_MS)
        .clamp(1, MAX_WAIT_TIMEOUT_MS)
}

pub(super) fn normalize_connect_dimension(value: Option<u32>, default_value: u32) -> u32 {
    value
        .unwrap_or(default_value)
        .clamp(1, MAX_CONNECT_DIMENSION)
}

pub(super) fn normalize_profile_type(connection_type: &str) -> String {
    connection_type.trim().to_ascii_lowercase()
}

pub(super) fn normalize_profile_host(profile: &SavedConnection) -> String {
    profile
        .host
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(super) fn normalize_profile_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn normalize_profile_encoding(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("shift-jis") => "shift-jis".into(),
        Some("euc-jp") => "euc-jp".into(),
        _ => "utf-8".into(),
    }
}

pub(super) fn normalize_profile_terminal_mode(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("cisco_ios") => "cisco_ios".into(),
        _ => "general".into(),
    }
}

pub(super) fn normalize_profile_auth_method(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("password") => Ok("password".into()),
        Some("public_key") => Ok("public_key".into()),
        Some(_) => Err("SSH認証方式が不正です".into()),
    }
}

pub(super) fn ssh_credential_required(
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

pub(super) fn available_serial_port_names(ports: &[serial::PortInfo]) -> String {
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

pub(super) fn normalize_serial_data_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_DATA_BITS);
    match value {
        5 | 6 | 7 | 8 => Ok(value),
        _ => Err("data_bits は 5, 6, 7, 8 のいずれかを指定してください".into()),
    }
}

pub(super) fn normalize_serial_stop_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_STOP_BITS);
    match value {
        1 | 2 => Ok(value),
        _ => Err("stop_bits は 1 または 2 を指定してください".into()),
    }
}

pub(super) fn prepare_serial_console_connection(
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

pub(super) fn list_connection_profiles_from_config(
    config: &AppConfig,
) -> Vec<McpConnectionProfile> {
    config
        .saved_connections
        .iter()
        .filter_map(|profile| {
            let connection_type = normalize_profile_type(&profile.connection_type);
            let memo = normalize_profile_string(profile.memo.as_deref());
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
                    jump_profile_id: normalize_profile_string(profile.jump_profile_id.as_deref()),
                    memo,
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
                    jump_profile_id: None,
                    memo,
                }),
                _ => None,
            }
        })
        .collect()
}

pub(super) fn prepare_saved_profile_connection(
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
            let jump_profile = ssh::resolve_jump_profile(
                config,
                profile.jump_profile_id.as_deref(),
                Some(profile.id.as_str()),
            )?;
            let port = profile.port.unwrap_or(22);
            Ok(PreparedConnection {
                kind: PreparedConnectionKind::Ssh {
                    host: host.clone(),
                    port,
                    username: username.clone(),
                    auth_method,
                    private_key_path,
                    jump_profile,
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
            jump_profile,
        } => {
            let credentials = runtime
                .credentials
                .as_ref()
                .ok_or_else(|| internal_error("MCP認証入力に必要な状態がありません"))?;
            let jump_credential = if let Some(jump_profile) = jump_profile {
                ssh::verify_trusted_host_key(&jump_profile.host, jump_profile.port)
                    .await
                    .map_err(invalid_params)?;
                request_profile_credential(
                    credentials,
                    app,
                    &jump_profile.id,
                    &jump_profile.host,
                    jump_profile.port,
                    &jump_profile.username,
                    &jump_profile.auth_method,
                    jump_profile.private_key_path.as_deref(),
                    &format!(
                        "{}@{}:{}",
                        jump_profile.username, jump_profile.host, jump_profile.port
                    ),
                    &format!("{}@{}", jump_profile.username, jump_profile.host),
                )
                .await?
            } else {
                None
            };

            if let Some(jump_profile) = jump_profile {
                let (jump_password, jump_key_passphrase) = if jump_profile.auth_method == "password"
                {
                    (jump_credential.clone(), None)
                } else {
                    (None, jump_credential.clone())
                };
                ssh::verify_trusted_host_key_via_jump(
                    host,
                    *port,
                    jump_profile.clone(),
                    jump_password,
                    jump_key_passphrase,
                )
                .await
                .map_err(invalid_params)?;
            } else {
                ssh::verify_trusted_host_key(host, *port)
                    .await
                    .map_err(invalid_params)?;
            }

            let credential = request_profile_credential(
                credentials,
                app,
                &prepared.profile_id,
                host,
                *port,
                username,
                auth_method,
                private_key_path.as_deref(),
                &prepared.target,
                &prepared.title,
            )
            .await?;
            let (password, key_passphrase) = if auth_method == "password" {
                (credential.unwrap_or_default(), None)
            } else {
                (String::new(), credential)
            };
            let (jump_password, jump_key_passphrase) = match jump_profile {
                Some(jump_profile) if jump_profile.auth_method == "password" => {
                    (jump_credential, None)
                }
                Some(_) => (None, jump_credential),
                None => (None, None),
            };

            ssh::connect(
                app,
                &runtime.ssh,
                &runtime.terminals,
                &runtime.workspace,
                runtime.logger.as_ref(),
                ssh::SshConnectOptions {
                    host: host.clone(),
                    port: *port,
                    username: username.clone(),
                    password,
                    auth_method: Some(auth_method.clone()),
                    private_key_path: private_key_path.clone(),
                    key_passphrase,
                    jump_profile_id: jump_profile.as_ref().map(|profile| profile.id.clone()),
                    jump_password,
                    jump_key_passphrase,
                    cols: prepared.cols,
                    rows: prepared.rows,
                    encoding: Some(prepared.encoding.clone()),
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
            &runtime.workspace,
            runtime.logger.as_ref(),
            host.clone(),
            *port,
            prepared.cols,
            prepared.rows,
            Some(prepared.encoding.clone()),
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
#[allow(clippy::too_many_arguments)]
async fn request_profile_credential(
    credentials: &McpCredentialState,
    app: &AppHandle,
    profile_id: &str,
    host: &str,
    port: u16,
    username: &str,
    auth_method: &str,
    private_key_path: Option<&str>,
    target: &str,
    title: &str,
) -> Result<Option<String>, McpError> {
    if !ssh_credential_required(auth_method, private_key_path).map_err(invalid_params)? {
        return Ok(None);
    }

    credentials
        .request_ssh_credential(
            app,
            McpCredentialRequestPayload {
                request_id: String::new(),
                profile_id: profile_id.to_string(),
                host: host.to_string(),
                port,
                username: username.to_string(),
                auth_method: auth_method.to_string(),
                target: target.to_string(),
                title: title.to_string(),
            },
        )
        .await
        .map_err(invalid_params)?
        .map(Some)
        .ok_or_else(|| invalid_params("MCP認証入力がキャンセルされました"))
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

    let protocol = terminal_protocol_from_log_type(&connection_type).map_err(invalid_params)?;
    let payload = McpConnectionCreatedPayload {
        session_id: session_id.clone(),
        connection_type: connection_type.clone(),
        target,
        title: title.clone(),
        encoding: encoding.clone(),
        terminal_mode: terminal_mode.clone(),
        auto_logging,
    };

    let snapshot = runtime
        .workspace
        .register_tab(WorkspaceTabRegisterInput {
            window_id: None,
            tab_id: None,
            session_id,
            connection_type: protocol,
            title,
            encoding,
            terminal_mode,
            is_auto_logging: auto_logging,
        })
        .await;
    emit_workspace_updated(app, &snapshot);

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
        &runtime.workspace,
        runtime.logger.as_ref(),
        prepared.port.clone(),
        prepared.config,
        Some(prepared.encoding.clone()),
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct McpConnectionProfile {
    pub(super) id: String,
    pub(super) connection_type: String,
    pub(super) host: String,
    pub(super) port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) terminal_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) private_key_configured: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) jump_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) memo: Option<String>,
}

#[cfg(not(test))]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct McpConnectionCreatedPayload {
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
pub(super) struct McpCredentialRequestPayload {
    pub(super) request_id: String,
    pub(super) profile_id: String,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) username: String,
    pub(super) auth_method: String,
    pub(super) target: String,
    pub(super) title: String,
}

#[cfg_attr(test, allow(dead_code))]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct McpLogControlRequestPayload {
    pub(super) request_id: String,
    pub(super) session_id: String,
    pub(super) connection_type: String,
    pub(super) target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct McpLogControlAck {
    pub(super) file_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct ConnectSavedProfileArgs {
    /// Saved profile ID from list_connection_profiles.
    pub(super) profile_id: String,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(super) cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(super) rows: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(super) enum McpSerialParity {
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

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(super) enum McpSerialFlowControl {
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

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(super) enum McpTerminalMode {
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct ConnectSerialConsoleArgs {
    /// Serial port name, for example COM3.
    pub(super) port: String,
    /// Baud rate. Defaults to 9600.
    #[schemars(range(min = 1))]
    pub(super) baud_rate: Option<u32>,
    /// Data bits. Supported values are 5, 6, 7, and 8. Defaults to 8.
    #[schemars(range(min = 5, max = 8))]
    pub(super) data_bits: Option<u8>,
    /// Parity. Defaults to none.
    pub(super) parity: Option<McpSerialParity>,
    /// Stop bits. Supported values are 1 and 2. Defaults to 1.
    #[schemars(range(min = 1, max = 2))]
    pub(super) stop_bits: Option<u8>,
    /// Flow control. Defaults to none.
    pub(super) flow_control: Option<McpSerialFlowControl>,
    /// Initial terminal mode. Defaults to general.
    pub(super) terminal_mode: Option<McpTerminalMode>,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(super) cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(super) rows: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedConnection {
    pub(super) kind: PreparedConnectionKind,
    pub(super) profile_id: String,
    pub(super) connection_type: String,
    pub(super) target: String,
    pub(super) title: String,
    pub(super) encoding: String,
    pub(super) terminal_mode: String,
    pub(super) cols: u32,
    pub(super) rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PreparedConnectionKind {
    Ssh {
        host: String,
        port: u16,
        username: String,
        auth_method: String,
        private_key_path: Option<String>,
        jump_profile: Option<ssh::SshJumpProfile>,
    },
    Telnet {
        host: String,
        port: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedSerialConnection {
    pub(super) port: String,
    pub(super) config: serial::SerialConfig,
    pub(super) target: String,
    pub(super) title: String,
    pub(super) encoding: String,
    pub(super) terminal_mode: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct ReadTerminalOutputArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(super) session_id: String,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    pub(super) max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct ReadTerminalOutputDeltaArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(super) session_id: String,
    /// Cursor returned by read_terminal_output, read_terminal_output_delta, wait_terminal_output, or run_terminal_command.
    pub(super) cursor: usize,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    pub(super) max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct WaitTerminalOutputArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(super) session_id: String,
    /// Cursor to wait from. When omitted, waits from the current output cursor.
    pub(super) cursor: Option<usize>,
    /// Optional substring to wait for in the output delta.
    pub(super) contains: Option<String>,
    /// Maximum wait time in milliseconds.
    #[schemars(range(min = 1, max = MAX_WAIT_TIMEOUT_MS))]
    pub(super) timeout_ms: Option<u64>,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    pub(super) max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct SendTerminalInputArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(super) session_id: String,
    /// Text to send to the terminal. Include newline characters when needed.
    pub(super) data: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct StartTerminalLogArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(super) session_id: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct StopTerminalLogArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(super) session_id: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(super) struct RunTerminalCommandArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(super) session_id: String,
    /// Command text to send to the terminal.
    pub(super) command: String,
    /// Append a newline after the command. Defaults to true.
    pub(super) append_newline: Option<bool>,
    /// Optional substring to wait for in the output delta after sending.
    pub(super) wait_contains: Option<String>,
    /// Maximum wait time in milliseconds.
    #[schemars(range(min = 1, max = MAX_WAIT_TIMEOUT_MS))]
    pub(super) timeout_ms: Option<u64>,
    /// Additional quiet period after a match before the final output delta is returned.
    #[schemars(range(min = 0, max = MAX_SETTLE_MS))]
    pub(super) settle_ms: Option<u64>,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    pub(super) max_chars: Option<usize>,
}
pub(super) fn structured_tool_result(value: Value) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| internal_error(format!("Serialize MCP tool result failed: {error}")))?;
    let mut result = CallToolResult::structured(value);
    result.content = vec![Content::text(text)];
    Ok(result)
}

pub(super) fn invalid_params(message: impl Into<String>) -> McpError {
    McpError::invalid_params(message.into(), None)
}

pub(super) fn internal_error(message: impl Into<String>) -> McpError {
    McpError::internal_error(message.into(), None)
}
