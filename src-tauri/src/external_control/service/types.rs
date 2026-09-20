use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::serial;
use crate::ssh;

use super::{MAX_CONNECT_DIMENSION, MAX_READ_CHARS, MAX_SETTLE_MS, MAX_WAIT_TIMEOUT_MS};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct ConnectSshArgs {
    /// SSH host name, IPv4 address, or IPv6 address. Do not include a scheme, user name, or port.
    pub(crate) host: String,
    /// SSH port. Defaults to 22.
    #[schemars(range(min = 1, max = 65535))]
    pub(crate) port: Option<u16>,
    /// SSH user name.
    pub(crate) username: String,
    /// SSH authentication method. Defaults to auto.
    pub(crate) auth_method: Option<ExternalControlSshAuthMethod>,
    /// Optional private key file path for auto or public-key authentication.
    pub(crate) private_key_path: Option<String>,
    /// Optional externally enabled saved SSH profile to use as a single jump host.
    pub(crate) jump_profile_id: Option<String>,
    /// Terminal character encoding. Defaults to UTF-8.
    pub(crate) encoding: Option<ExternalControlEncoding>,
    /// Initial terminal mode. Defaults to general.
    pub(crate) terminal_mode: Option<ExternalControlTerminalMode>,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) rows: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
pub(crate) struct ConnectTelnetArgs {
    /// Telnet host name, IPv4 address, or IPv6 address. Do not include a scheme or port.
    pub(crate) host: String,
    /// Telnet port. Defaults to 23.
    #[schemars(range(min = 1, max = 65535))]
    pub(crate) port: Option<u16>,
    /// Terminal character encoding. Defaults to UTF-8.
    pub(crate) encoding: Option<ExternalControlEncoding>,
    /// Initial terminal mode. Defaults to general.
    pub(crate) terminal_mode: Option<ExternalControlTerminalMode>,
    /// Requested terminal columns. Defaults to 120.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) cols: Option<u32>,
    /// Requested terminal rows. Defaults to 30.
    #[schemars(range(min = 1, max = MAX_CONNECT_DIMENSION))]
    pub(crate) rows: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExternalControlSshAuthMethod {
    #[default]
    Auto,
    Password,
    KeyboardInteractive,
    PublicKey,
}

impl ExternalControlSshAuthMethod {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Password => "password",
            Self::KeyboardInteractive => "keyboard_interactive",
            Self::PublicKey => "public_key",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ExternalControlEncoding {
    #[default]
    Utf8,
    ShiftJis,
    EucJp,
}

impl ExternalControlEncoding {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Utf8 => "utf-8",
            Self::ShiftJis => "shift-jis",
            Self::EucJp => "euc-jp",
        }
    }
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
    pub(crate) fn as_str(&self) -> &'static str {
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
    pub(crate) fn as_str(&self) -> &'static str {
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
    pub(crate) fn as_str(&self) -> &'static str {
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
    AristaEos,
    JuniperJunos,
    Vyos,
    FujitsuSir,
    AlliedTelesisAwplus,
    FurukawaFitelnet,
}

impl ExternalControlTerminalMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::CiscoIos => "cisco_ios",
            Self::AristaEos => "arista_eos",
            Self::JuniperJunos => "juniper_junos",
            Self::Vyos => "vyos",
            Self::FujitsuSir => "fujitsu_sir",
            Self::AlliedTelesisAwplus => "allied_telesis_awplus",
            Self::FurukawaFitelnet => "furukawa_fitelnet",
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
#[schemars(extend("type" = "object"))]
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
    ConnectSsh(ConnectSshArgs),
    ConnectTelnet(ConnectTelnetArgs),
    ListSerialPorts,
    ConnectSerialConsole(ConnectSerialConsoleArgs),
    ReadTerminalOutput(ReadTerminalOutputArgs),
    SendTerminalInput(SendTerminalInputArgs),
    StartTerminalLog(StartTerminalLogArgs),
    StopTerminalLog(StopTerminalLogArgs),
    RunTerminalCommand(RunTerminalCommandArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListTerminalSessionsResult {
    pub sessions: Vec<crate::terminal_control::TerminalSessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListConnectionProfilesResult {
    pub(crate) profiles: Vec<ExternalControlConnectionProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionCreatedResult {
    pub session_id: String,
    pub connection_type: String,
    pub target: String,
    pub title: String,
    pub encoding: String,
    pub terminal_mode: String,
    pub auto_logging: bool,
}

pub type ConnectSavedProfileResult = ConnectionCreatedResult;
pub type ConnectSshResult = ConnectionCreatedResult;
pub type ConnectTelnetResult = ConnectionCreatedResult;
pub type ConnectSerialConsoleResult = ConnectionCreatedResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListSerialPortsResult {
    pub ports: Vec<serial::PortInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalOutputResult {
    pub session_id: String,
    pub output: String,
    pub truncated: bool,
    pub available_chars: usize,
    pub start_cursor: usize,
    pub cursor: usize,
}

impl From<crate::terminal_control::TerminalOutputSnapshot> for TerminalOutputResult {
    fn from(snapshot: crate::terminal_control::TerminalOutputSnapshot) -> Self {
        Self {
            session_id: snapshot.session_id,
            output: snapshot.output,
            truncated: snapshot.truncated,
            available_chars: snapshot.available_chars,
            start_cursor: snapshot.start_cursor,
            cursor: snapshot.cursor,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaitTerminalOutputResult {
    pub session_id: String,
    pub matched: bool,
    pub timed_out: bool,
    pub output: String,
    pub truncated: bool,
    pub available_chars: usize,
    pub start_cursor: usize,
    pub cursor: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ReadTerminalOutputResult {
    Recent(TerminalOutputResult),
    Delta(TerminalOutputResult),
    Wait(WaitTerminalOutputResult),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendTerminalInputResult {
    pub session_id: String,
    pub sent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartTerminalLogResult {
    pub session_id: String,
    pub started: bool,
    pub already_active: bool,
    pub file_path: String,
    pub log_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopTerminalLogResult {
    pub session_id: String,
    pub stopped: bool,
    pub already_inactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunTerminalCommandResult {
    pub session_id: String,
    pub sent: bool,
    pub matched: bool,
    pub timed_out: bool,
    pub output: String,
    pub truncated: bool,
    pub available_chars: usize,
    pub start_cursor: usize,
    pub cursor: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum ExternalControlResponse {
    ListTerminalSessions(ListTerminalSessionsResult),
    ListConnectionProfiles(ListConnectionProfilesResult),
    ConnectSavedProfile(ConnectSavedProfileResult),
    ConnectSsh(ConnectSshResult),
    ConnectTelnet(ConnectTelnetResult),
    ListSerialPorts(ListSerialPortsResult),
    ConnectSerialConsole(ConnectSerialConsoleResult),
    ReadTerminalOutput(ReadTerminalOutputResult),
    SendTerminalInput(SendTerminalInputResult),
    StartTerminalLog(StartTerminalLogResult),
    StopTerminalLog(StopTerminalLogResult),
    RunTerminalCommand(RunTerminalCommandResult),
}

impl ExternalControlResponse {
    pub fn into_value(self) -> Result<Value, ExternalControlError> {
        let value = match self {
            Self::ListTerminalSessions(result) => serde_json::to_value(result),
            Self::ListConnectionProfiles(result) => serde_json::to_value(result),
            Self::ConnectSavedProfile(result) => serde_json::to_value(result),
            Self::ConnectSsh(result) => serde_json::to_value(result),
            Self::ConnectTelnet(result) => serde_json::to_value(result),
            Self::ListSerialPorts(result) => serde_json::to_value(result),
            Self::ConnectSerialConsole(result) => serde_json::to_value(result),
            Self::ReadTerminalOutput(result) => serde_json::to_value(result),
            Self::SendTerminalInput(result) => serde_json::to_value(result),
            Self::StartTerminalLog(result) => serde_json::to_value(result),
            Self::StopTerminalLog(result) => serde_json::to_value(result),
            Self::RunTerminalCommand(result) => serde_json::to_value(result),
        };
        value.map_err(|error| {
            internal_error(format!(
                "Failed to serialize the external control result: {error}"
            ))
        })
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

pub(crate) fn invalid_params(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::InvalidArguments(message.into())
}

pub(crate) fn permission_denied(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::PermissionDenied(message.into())
}

pub(crate) fn not_found(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::NotFound(message.into())
}

pub(crate) fn unavailable(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::Unavailable(message.into())
}

pub(crate) fn internal_error(message: impl Into<String>) -> ExternalControlError {
    ExternalControlError::Internal(message.into())
}
