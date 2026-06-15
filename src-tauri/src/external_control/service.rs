use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(not(test))]
use tauri::AppHandle;
use tokio::time;
#[cfg(not(test))]
use uuid::Uuid;

use crate::config::{self, AppConfig, SavedConnection};
#[cfg(not(test))]
use crate::external_control::protocol::ExternalControlCredentialState;
use crate::external_control::protocol::ExternalControlLogControlState;
use crate::logger::{self, LoggerState};
use crate::serial::{self, SerialState};
use crate::ssh::{self, SshState};
use crate::telnet::{self, TelnetState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};
use crate::workspace::WorkspaceState;
#[cfg(not(test))]
use crate::workspace::{emit_workspace_updated, WorkspaceTabRegisterInput};

pub(crate) const DEFAULT_READ_CHARS: usize = 2_000;
pub(crate) const MAX_READ_CHARS: usize = 20_000;
const MAX_INPUT_CHARS: usize = 20_000;
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 10_000;
pub(crate) const MAX_WAIT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_SETTLE_MS: u64 = 250;
const MAX_SETTLE_MS: u64 = 5_000;
const DEFAULT_CONNECT_COLS: u32 = 120;
const DEFAULT_CONNECT_ROWS: u32 = 30;
pub(crate) const MAX_CONNECT_DIMENSION: u32 = 1_000;
const DEFAULT_SERIAL_BAUD_RATE: u32 = 9_600;
const DEFAULT_SERIAL_DATA_BITS: u8 = 8;
const DEFAULT_SERIAL_STOP_BITS: u8 = 1;

#[derive(Clone)]
pub struct ExternalControlRuntime {
    #[cfg_attr(not(test), allow(dead_code))]
    pub config: ExternalControlPermissions,
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
    pub log_control: Option<ExternalControlLogControlState>,
    #[cfg(not(test))]
    pub credentials: Option<ExternalControlCredentialState>,
}

#[derive(Debug, Clone, Default)]
pub struct ExternalControlPermissions {
    #[cfg_attr(not(test), allow(dead_code))]
    pub connect_enabled: bool,
}

impl ExternalControlPermissions {
    pub fn new(connect_enabled: bool) -> Self {
        Self { connect_enabled }
    }
}

#[derive(Clone)]
pub struct ExternalControlService {
    pub(crate) runtime: ExternalControlRuntime,
}

impl ExternalControlService {
    pub fn new(runtime: ExternalControlRuntime) -> Self {
        Self { runtime }
    }

    pub async fn execute(
        &self,
        request: ExternalControlRequest,
    ) -> Result<ExternalControlResponse, ExternalControlError> {
        match request {
            ExternalControlRequest::ListTerminalSessions => self
                .list_terminal_sessions()
                .await
                .map(ListTerminalSessionsResult)
                .map(ExternalControlResponse::ListTerminalSessions),
            ExternalControlRequest::ListConnectionProfiles(args) => self
                .list_connection_profiles(args)
                .await
                .map(ListConnectionProfilesResult)
                .map(ExternalControlResponse::ListConnectionProfiles),
            ExternalControlRequest::ConnectSavedProfile(args) => self
                .connect_saved_profile(args)
                .await
                .map(ConnectSavedProfileResult)
                .map(ExternalControlResponse::ConnectSavedProfile),
            ExternalControlRequest::ListSerialPorts => self
                .list_serial_ports()
                .await
                .map(ListSerialPortsResult)
                .map(ExternalControlResponse::ListSerialPorts),
            ExternalControlRequest::ConnectSerialConsole(args) => self
                .connect_serial_console(args)
                .await
                .map(ConnectSerialConsoleResult)
                .map(ExternalControlResponse::ConnectSerialConsole),
            ExternalControlRequest::ReadTerminalOutput(args) => self
                .read_terminal_output(args)
                .await
                .map(ReadTerminalOutputResult)
                .map(ExternalControlResponse::ReadTerminalOutput),
            ExternalControlRequest::SendTerminalInput(args) => self
                .send_terminal_input(args)
                .await
                .map(SendTerminalInputResult)
                .map(ExternalControlResponse::SendTerminalInput),
            ExternalControlRequest::StartTerminalLog(args) => self
                .start_terminal_log(args)
                .await
                .map(StartTerminalLogResult)
                .map(ExternalControlResponse::StartTerminalLog),
            ExternalControlRequest::StopTerminalLog(args) => self
                .stop_terminal_log(args)
                .await
                .map(StopTerminalLogResult)
                .map(ExternalControlResponse::StopTerminalLog),
            ExternalControlRequest::RunTerminalCommand(args) => self
                .run_terminal_command(args)
                .await
                .map(RunTerminalCommandResult)
                .map(ExternalControlResponse::RunTerminalCommand),
        }
    }

    pub(crate) async fn list_terminal_sessions(&self) -> Result<Value, ExternalControlError> {
        let sessions = self.runtime.terminals.list_sessions().await;
        Ok(json!({
            "sessions": sessions,
        }))
    }

    fn ensure_connect_enabled(&self) -> Result<(), ExternalControlError> {
        if self.connect_enabled_now()? {
            Ok(())
        } else {
            Err(permission_denied(
                "外部制御からの新規接続は無効です。external_control.connect_enabled=true にしてください",
            ))
        }
    }

    fn connect_enabled_now(&self) -> Result<bool, ExternalControlError> {
        #[cfg(test)]
        {
            Ok(self.runtime.config.connect_enabled)
        }

        #[cfg(not(test))]
        {
            config::config_read()
                .map(|config| config.external_control.connect_enabled)
                .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))
        }
    }

    pub(crate) async fn list_connection_profiles(
        &self,
        args: ListConnectionProfilesArgs,
    ) -> Result<Value, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        Ok(json!({
            "profiles": list_connection_profiles_from_config(&config, args.connection_type),
        }))
    }

    pub(crate) async fn connect_saved_profile(
        &self,
        args: ConnectSavedProfileArgs,
    ) -> Result<Value, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        let prepared = prepare_saved_profile_connection(&config, args).map_err(invalid_params)?;
        connect_prepared_profile(&self.runtime, &config, prepared).await
    }

    pub(crate) async fn list_serial_ports(&self) -> Result<Value, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let ports = serial::serial_list_ports().map_err(internal_error)?;
        Ok(json!({
            "ports": ports,
        }))
    }

    pub(crate) async fn connect_serial_console(
        &self,
        args: ConnectSerialConsoleArgs,
    ) -> Result<Value, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let ports = serial::serial_list_ports().map_err(internal_error)?;
        let prepared = prepare_serial_console_connection(args, &ports).map_err(invalid_params)?;
        let config = config::config_read()
            .map_err(|error| internal_error(format!("設定読み込みエラー: {error}")))?;
        connect_prepared_serial_console(&self.runtime, &config, prepared).await
    }

    pub(crate) async fn read_terminal_output(
        &self,
        args: ReadTerminalOutputArgs,
    ) -> Result<Value, ExternalControlError> {
        match args {
            ReadTerminalOutputArgs::Recent {
                session_id,
                max_chars,
            } => {
                let snapshot = self
                    .runtime
                    .terminals
                    .read_output(&session_id, normalize_max_chars(max_chars))
                    .await
                    .map_err(invalid_params)?;

                Ok(json!({
                    "session_id": snapshot.session_id,
                    "mode": "recent",
                    "output": snapshot.output,
                    "truncated": snapshot.truncated,
                    "available_chars": snapshot.available_chars,
                    "start_cursor": snapshot.start_cursor,
                    "cursor": snapshot.cursor,
                }))
            }
            ReadTerminalOutputArgs::Delta {
                session_id,
                cursor,
                max_chars,
            } => {
                let snapshot = self
                    .runtime
                    .terminals
                    .read_output_delta(&session_id, cursor, normalize_max_chars(max_chars))
                    .await
                    .map_err(invalid_params)?;

                Ok(json!({
                    "session_id": snapshot.session_id,
                    "mode": "delta",
                    "output": snapshot.output,
                    "truncated": snapshot.truncated,
                    "available_chars": snapshot.available_chars,
                    "start_cursor": snapshot.start_cursor,
                    "cursor": snapshot.cursor,
                }))
            }
            ReadTerminalOutputArgs::Wait {
                session_id,
                cursor,
                contains,
                timeout_ms,
                max_chars,
            } => {
                let start_cursor = match cursor {
                    Some(cursor) => cursor,
                    None => self
                        .runtime
                        .terminals
                        .cursor(&session_id)
                        .await
                        .map_err(invalid_params)?,
                };
                let contains = contains.filter(|value| !value.is_empty());
                let mut result = wait_for_terminal_output(
                    &self.runtime.terminals,
                    &session_id,
                    start_cursor,
                    contains.as_deref(),
                    normalize_max_chars(max_chars),
                    normalize_timeout_ms(timeout_ms),
                )
                .await?;
                result["mode"] = json!("wait");
                Ok(result)
            }
        }
    }

    pub(crate) async fn send_terminal_input(
        &self,
        args: SendTerminalInputArgs,
    ) -> Result<Value, ExternalControlError> {
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, args.data).await?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
        }))
    }

    pub(crate) async fn start_terminal_log(
        &self,
        args: StartTerminalLogArgs,
    ) -> Result<Value, ExternalControlError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| not_found("セッションが見つかりません"))?;

        if info.status != TerminalStatus::Connected {
            return Err(unavailable("セッションは切断済みです"));
        }

        let logger_state = self
            .runtime
            .logger
            .as_ref()
            .ok_or_else(|| internal_error("外部制御ログ開始に必要なロガー状態がありません"))?;
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

    pub(crate) async fn stop_terminal_log(
        &self,
        args: StopTerminalLogArgs,
    ) -> Result<Value, ExternalControlError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| not_found("セッションが見つかりません"))?;

        let logger_state = self
            .runtime
            .logger
            .as_ref()
            .ok_or_else(|| internal_error("外部制御ログ停止に必要なロガー状態がありません"))?;
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

    pub(crate) async fn run_terminal_command(
        &self,
        args: RunTerminalCommandArgs,
    ) -> Result<Value, ExternalControlError> {
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

#[cfg(not(test))]
async fn request_manual_log_start(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<String, String> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| "外部制御ログ開始に必要なアプリハンドルがありません".to_string())?;
    let log_control = runtime
        .log_control
        .as_ref()
        .ok_or_else(|| "外部制御ログ開始に必要なログ制御状態がありません".to_string())?;
    let ack = log_control
        .request(
            app,
            "external-control://log-start-request",
            ExternalControlLogControlRequestPayload {
                request_id: Uuid::new_v4().to_string(),
                session_id: info.session_id.clone(),
                connection_type: terminal_protocol_log_type(info.protocol).into(),
                target: info.target.clone(),
            },
        )
        .await?;
    ack.file_path
        .ok_or_else(|| "外部制御ログ開始応答にログファイルパスがありません".to_string())
}

#[cfg(test)]
async fn request_manual_log_start(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<String, String> {
    let logger_state = runtime
        .logger
        .as_ref()
        .ok_or_else(|| "外部制御ログ開始に必要なロガー状態がありません".to_string())?;
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
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<(), String> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| "外部制御ログ停止に必要なアプリハンドルがありません".to_string())?;
    let log_control = runtime
        .log_control
        .as_ref()
        .ok_or_else(|| "外部制御ログ停止に必要なログ制御状態がありません".to_string())?;
    log_control
        .request(
            app,
            "external-control://log-stop-request",
            ExternalControlLogControlRequestPayload {
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
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<(), String> {
    let logger_state = runtime
        .logger
        .as_ref()
        .ok_or_else(|| "外部制御ログ停止に必要なロガー状態がありません".to_string())?;
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
    runtime: &ExternalControlRuntime,
    session_id: &str,
    data: String,
) -> Result<(), ExternalControlError> {
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
        .ok_or_else(|| not_found("セッションが見つかりません"))?;

    if info.status != TerminalStatus::Connected {
        return Err(unavailable("セッションは切断済みです"));
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
) -> Result<Value, ExternalControlError> {
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

pub(crate) fn normalize_max_chars(max_chars: Option<usize>) -> usize {
    max_chars
        .unwrap_or(DEFAULT_READ_CHARS)
        .clamp(1, MAX_READ_CHARS)
}

pub(crate) fn normalize_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(DEFAULT_WAIT_TIMEOUT_MS)
        .clamp(1, MAX_WAIT_TIMEOUT_MS)
}

pub(crate) fn normalize_connect_dimension(value: Option<u32>, default_value: u32) -> u32 {
    value
        .unwrap_or(default_value)
        .clamp(1, MAX_CONNECT_DIMENSION)
}

pub(crate) fn normalize_profile_type(connection_type: &str) -> String {
    connection_type.trim().to_ascii_lowercase()
}

pub(crate) fn normalize_profile_host(profile: &SavedConnection) -> String {
    profile
        .host
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn normalize_profile_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn profile_external_control_enabled(profile: &SavedConnection) -> bool {
    profile.external_control_enabled
}

pub(crate) fn normalize_profile_encoding(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("shift-jis") => "shift-jis".into(),
        Some("euc-jp") => "euc-jp".into(),
        _ => "utf-8".into(),
    }
}

pub(crate) fn normalize_profile_terminal_mode(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("cisco_ios") => "cisco_ios".into(),
        _ => "general".into(),
    }
}

pub(crate) fn normalize_profile_auth_method(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("password") => Ok("password".into()),
        Some("public_key") => Ok("public_key".into()),
        Some(_) => Err("SSH認証方式が不正です".into()),
    }
}

pub(crate) fn ssh_credential_required(
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

pub(crate) fn available_serial_port_names(ports: &[serial::PortInfo]) -> String {
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

pub(crate) fn normalize_serial_data_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_DATA_BITS);
    match value {
        5 | 6 | 7 | 8 => Ok(value),
        _ => Err("data_bits は 5, 6, 7, 8 のいずれかを指定してください".into()),
    }
}

pub(crate) fn normalize_serial_stop_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_STOP_BITS);
    match value {
        1 | 2 => Ok(value),
        _ => Err("stop_bits は 1 または 2 を指定してください".into()),
    }
}

pub(crate) fn prepare_serial_console_connection(
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

pub(crate) fn list_connection_profiles_from_config(
    config: &AppConfig,
    connection_type_filter: Option<SavedProfileConnectionType>,
) -> Vec<ExternalControlConnectionProfile> {
    config
        .saved_connections
        .iter()
        .filter_map(|profile| {
            if !profile_external_control_enabled(profile) {
                return None;
            }
            let connection_type = normalize_profile_type(&profile.connection_type);
            if connection_type_filter_mismatch(&connection_type, connection_type_filter.as_ref()) {
                return None;
            }
            let memo = normalize_profile_string(profile.memo.as_deref());
            match connection_type.as_str() {
                "ssh" => Some(ExternalControlConnectionProfile {
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
                "telnet" => Some(ExternalControlConnectionProfile {
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

fn connection_type_filter_mismatch(
    profile_type: &str,
    filter: Option<&SavedProfileConnectionType>,
) -> bool {
    filter.is_some_and(|filter| profile_type != filter.as_str())
}

pub(crate) fn prepare_saved_profile_connection(
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
        .find(|profile| {
            profile.id == profile_id
                && normalize_profile_type(&profile.connection_type) == args.connection_type.as_str()
        })
        .ok_or_else(|| "保存済みプロファイルが見つかりません".to_string())?;
    if !profile_external_control_enabled(profile) {
        return Err("この保存済みプロファイルは外部制御からの利用が無効です".into());
    }
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
        _ => Err("外部制御の新規接続は保存済みSSH/Telnetプロファイルのみ対応しています".into()),
    }
}

#[cfg(not(test))]
async fn connect_prepared_profile(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedConnection,
) -> Result<Value, ExternalControlError> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| internal_error("外部制御接続に必要なアプリハンドルがありません"))?;

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
                .ok_or_else(|| internal_error("外部制御の認証入力に必要な状態がありません"))?;
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
                    request_id: None,
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
    credentials: &ExternalControlCredentialState,
    app: &AppHandle,
    profile_id: &str,
    host: &str,
    port: u16,
    username: &str,
    auth_method: &str,
    private_key_path: Option<&str>,
    target: &str,
    title: &str,
) -> Result<Option<String>, ExternalControlError> {
    if !ssh_credential_required(auth_method, private_key_path).map_err(invalid_params)? {
        return Ok(None);
    }

    credentials
        .request_ssh_credential(
            app,
            ExternalControlCredentialRequestPayload {
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
        .ok_or_else(|| invalid_params("外部制御の認証入力がキャンセルされました"))
}

#[cfg(not(test))]
async fn finish_created_session(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    session_id: String,
    connection_type: String,
    target: String,
    title: String,
    encoding: String,
    terminal_mode: String,
) -> Result<Value, ExternalControlError> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| internal_error("外部制御接続に必要なアプリハンドルがありません"))?;
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
                log::warn!(
                    "External control auto log start failed for session {session_id}: {error}"
                );
                false
            }),
            None => {
                log::warn!(
                    "External control auto log start skipped because logger state is unavailable"
                );
                false
            }
        }
    } else {
        false
    };

    let protocol = terminal_protocol_from_log_type(&connection_type).map_err(invalid_params)?;
    let payload = ExternalControlConnectionCreatedPayload {
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
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedSerialConnection,
) -> Result<Value, ExternalControlError> {
    let app = runtime
        .app
        .as_ref()
        .ok_or_else(|| internal_error("外部制御接続に必要なアプリハンドルがありません"))?;

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
    _runtime: &ExternalControlRuntime,
    _config: &AppConfig,
    _prepared: PreparedConnection,
) -> Result<Value, ExternalControlError> {
    Err(internal_error(
        "外部制御プロファイル接続の実接続処理はユニットテストでは実行しません",
    ))
}

#[cfg(test)]
async fn connect_prepared_serial_console(
    _runtime: &ExternalControlRuntime,
    _config: &AppConfig,
    _prepared: PreparedSerialConnection,
) -> Result<Value, ExternalControlError> {
    Err(internal_error(
        "外部制御シリアル接続の実接続処理はユニットテストでは実行しません",
    ))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ExternalControlConnectionProfile {
    pub(crate) id: String,
    pub(crate) connection_type: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) auth_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) terminal_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) private_key_configured: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) jump_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) memo: Option<String>,
}

#[cfg(not(test))]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ExternalControlConnectionCreatedPayload {
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
pub(crate) struct ExternalControlCredentialRequestPayload {
    pub(crate) request_id: String,
    pub(crate) profile_id: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) username: String,
    pub(crate) auth_method: String,
    pub(crate) target: String,
    pub(crate) title: String,
}

#[cfg_attr(test, allow(dead_code))]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct ExternalControlLogControlRequestPayload {
    pub(crate) request_id: String,
    pub(crate) session_id: String,
    pub(crate) connection_type: String,
    pub(crate) target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalControlLogControlAck {
    pub(crate) file_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct ConnectSavedProfileArgs {
    /// Saved profile ID from list_connection_profiles.
    pub(crate) profile_id: String,
    /// Saved profile connection type.
    pub(crate) connection_type: SavedProfileConnectionType,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) rows: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct ListConnectionProfilesArgs {
    pub(crate) connection_type: Option<SavedProfileConnectionType>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SavedProfileConnectionType {
    Ssh,
    Telnet,
}

impl SavedProfileConnectionType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Telnet => "telnet",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExternalControlSerialParity {
    #[default]
    None,
    Odd,
    Even,
}

impl ExternalControlSerialParity {
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
pub(crate) enum ExternalControlSerialFlowControl {
    #[default]
    None,
    Software,
    Hardware,
}

impl ExternalControlSerialFlowControl {
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
pub(crate) enum ExternalControlTerminalMode {
    #[default]
    General,
    CiscoIos,
}

impl ExternalControlTerminalMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::CiscoIos => "cisco_ios",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct ConnectSerialConsoleArgs {
    /// Serial port name, for example COM3.
    pub(crate) port: String,
    /// Baud rate. Defaults to 9600.
    #[schemars(range(min = 1))]
    pub(crate) baud_rate: Option<u32>,
    /// Data bits. Supported values are 5, 6, 7, and 8. Defaults to 8.
    #[schemars(range(min = 5, max = 8))]
    pub(crate) data_bits: Option<u8>,
    /// Parity. Defaults to none.
    pub(crate) parity: Option<ExternalControlSerialParity>,
    /// Stop bits. Supported values are 1 and 2. Defaults to 1.
    #[schemars(range(min = 1, max = 2))]
    pub(crate) stop_bits: Option<u8>,
    /// Flow control. Defaults to none.
    pub(crate) flow_control: Option<ExternalControlSerialFlowControl>,
    /// Initial terminal mode. Defaults to general.
    pub(crate) terminal_mode: Option<ExternalControlTerminalMode>,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) rows: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedConnection {
    pub(crate) kind: PreparedConnectionKind,
    pub(crate) profile_id: String,
    pub(crate) connection_type: String,
    pub(crate) target: String,
    pub(crate) title: String,
    pub(crate) encoding: String,
    pub(crate) terminal_mode: String,
    pub(crate) cols: u32,
    pub(crate) rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreparedConnectionKind {
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
pub(crate) struct PreparedSerialConnection {
    pub(crate) port: String,
    pub(crate) config: serial::SerialConfig,
    pub(crate) target: String,
    pub(crate) title: String,
    pub(crate) encoding: String,
    pub(crate) terminal_mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ReadTerminalOutputArgs {
    /// Read the most recent output retained for the session.
    Recent {
        /// Session ID returned by list_terminal_sessions.
        session_id: String,
        /// Maximum number of recent characters to return.
        #[schemars(range(min = 1, max = MAX_READ_CHARS))]
        max_chars: Option<usize>,
    },
    /// Read output written after a previously returned cursor.
    Delta {
        /// Session ID returned by list_terminal_sessions.
        session_id: String,
        /// Cursor returned by read_terminal_output or run_terminal_command.
        cursor: usize,
        /// Maximum number of recent characters to return.
        #[schemars(range(min = 1, max = MAX_READ_CHARS))]
        max_chars: Option<usize>,
    },
    /// Wait for new output or for a substring to appear.
    Wait {
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
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct SendTerminalInputArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(crate) session_id: String,
    /// Text to send to the terminal. Include newline characters when needed.
    pub(crate) data: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct StartTerminalLogArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(crate) session_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct StopTerminalLogArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(crate) session_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct RunTerminalCommandArgs {
    /// Session ID returned by list_terminal_sessions.
    pub(crate) session_id: String,
    /// Command text to send to the terminal.
    pub(crate) command: String,
    /// Append a newline after the command. Defaults to true.
    pub(crate) append_newline: Option<bool>,
    /// Optional substring to wait for in the output delta after sending.
    pub(crate) wait_contains: Option<String>,
    /// Maximum wait time in milliseconds.
    #[schemars(range(min = 1, max = MAX_WAIT_TIMEOUT_MS))]
    pub(crate) timeout_ms: Option<u64>,
    /// Additional quiet period after a match before the final output delta is returned.
    #[schemars(range(min = 0, max = MAX_SETTLE_MS))]
    pub(crate) settle_ms: Option<u64>,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    pub(crate) max_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", content = "arguments", rename_all = "snake_case")]
pub enum ExternalControlRequest {
    ListTerminalSessions,
    ListConnectionProfiles(ListConnectionProfilesArgs),
    ConnectSavedProfile(ConnectSavedProfileArgs),
    ListSerialPorts,
    ConnectSerialConsole(ConnectSerialConsoleArgs),
    ReadTerminalOutput(ReadTerminalOutputArgs),
    SendTerminalInput(SendTerminalInputArgs),
    StartTerminalLog(StartTerminalLogArgs),
    StopTerminalLog(StopTerminalLogArgs),
    RunTerminalCommand(RunTerminalCommandArgs),
}

macro_rules! result_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        #[serde(transparent)]
        pub struct $name(pub Value);
    };
}

result_type!(ListTerminalSessionsResult);
result_type!(ListConnectionProfilesResult);
result_type!(ConnectSavedProfileResult);
result_type!(ListSerialPortsResult);
result_type!(ConnectSerialConsoleResult);
result_type!(ReadTerminalOutputResult);
result_type!(SendTerminalInputResult);
result_type!(StartTerminalLogResult);
result_type!(StopTerminalLogResult);
result_type!(RunTerminalCommandResult);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum ExternalControlResponse {
    ListTerminalSessions(ListTerminalSessionsResult),
    ListConnectionProfiles(ListConnectionProfilesResult),
    ConnectSavedProfile(ConnectSavedProfileResult),
    ListSerialPorts(ListSerialPortsResult),
    ConnectSerialConsole(ConnectSerialConsoleResult),
    ReadTerminalOutput(ReadTerminalOutputResult),
    SendTerminalInput(SendTerminalInputResult),
    StartTerminalLog(StartTerminalLogResult),
    StopTerminalLog(StopTerminalLogResult),
    RunTerminalCommand(RunTerminalCommandResult),
}

impl ExternalControlResponse {
    pub fn into_value(self) -> Value {
        match self {
            Self::ListTerminalSessions(result) => result.0,
            Self::ListConnectionProfiles(result) => result.0,
            Self::ConnectSavedProfile(result) => result.0,
            Self::ListSerialPorts(result) => result.0,
            Self::ConnectSerialConsole(result) => result.0,
            Self::ReadTerminalOutput(result) => result.0,
            Self::SendTerminalInput(result) => result.0,
            Self::StartTerminalLog(result) => result.0,
            Self::StopTerminalLog(result) => result.0,
            Self::RunTerminalCommand(result) => result.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum ExternalControlError {
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unavailable: {0}")]
    Unavailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl ExternalControlError {
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidArguments(message)
            | Self::PermissionDenied(message)
            | Self::NotFound(message)
            | Self::Unavailable(message)
            | Self::Internal(message) => message,
        }
    }
}

fn invalid_params(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::InvalidArguments(message.into())
}

fn permission_denied(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::PermissionDenied(message.into())
}

fn not_found(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::NotFound(message.into())
}

fn unavailable(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::Unavailable(message.into())
}

fn internal_error(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::Internal(message.into())
}
