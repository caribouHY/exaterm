#[cfg(not(test))]
use tauri::AppHandle;

#[cfg(not(test))]
use crate::config;
use crate::config::AppConfig;
#[cfg(not(test))]
use crate::external_control::protocol::ExternalControlCredentialState;
use crate::external_control::protocol::ExternalControlLogControlState;
use crate::logger::LoggerState;
use crate::serial::SerialState;
use crate::ssh::SshState;
use crate::telnet::TelnetState;
use crate::terminal_control::TerminalControlState;
use crate::workspace::WorkspaceState;

mod connections;
mod profiles;
mod terminal;
mod types;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use profiles::{
    available_serial_port_names, list_connection_profiles_from_config, normalize_connect_dimension,
    normalize_profile_auth_method, normalize_profile_encoding, normalize_profile_host,
    normalize_profile_string, normalize_profile_terminal_mode, normalize_profile_type,
    normalize_serial_data_bits, normalize_serial_stop_bits, prepare_saved_profile_connection,
    prepare_serial_console_connection, profile_external_control_enabled, ssh_credential_required,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use terminal::{normalize_max_chars, normalize_timeout_ms};
pub(crate) use types::ExternalControlConnectionCreatedPayload;
pub(super) use types::{internal_error, invalid_params, not_found, permission_denied, unavailable};
pub(crate) use types::{
    ConnectSavedProfileArgs, ConnectSerialConsoleArgs, ExternalControlConnectionProfile,
    ExternalControlLogControlAck, ExternalControlSerialFlowControl, ExternalControlSerialParity,
    ExternalControlTerminalMode, ListConnectionProfilesArgs, PreparedConnection,
    PreparedConnectionKind, PreparedSerialConnection, ReadTerminalOutputArgs,
    RunTerminalCommandArgs, SavedProfileConnectionType, SendTerminalInputArgs,
    StartTerminalLogArgs, StopTerminalLogArgs,
};
use types::{
    ConnectSavedProfileResult, ConnectSerialConsoleResult, ListConnectionProfilesResult,
    ListSerialPortsResult, ReadTerminalOutputResult, RunTerminalCommandResult,
    SendTerminalInputResult, StartTerminalLogResult, StopTerminalLogResult,
};
#[cfg(not(test))]
pub(crate) use types::{
    ExternalControlCredentialRequestPayload, ExternalControlLogControlRequestPayload,
};
pub use types::{
    ExternalControlError, ExternalControlRequest, ExternalControlResponse,
    ListTerminalSessionsResult,
};

pub(crate) const DEFAULT_READ_CHARS: usize = 2_000;
pub(crate) const MAX_READ_CHARS: usize = 20_000;
pub(super) const MAX_INPUT_CHARS: usize = 20_000;
pub(super) const DEFAULT_WAIT_TIMEOUT_MS: u64 = 10_000;
pub(crate) const MAX_WAIT_TIMEOUT_MS: u64 = 60_000;
pub(super) const DEFAULT_SETTLE_MS: u64 = 250;
pub(super) const MAX_SETTLE_MS: u64 = 5_000;
pub(super) const DEFAULT_CONNECT_COLS: u32 = 120;
pub(super) const DEFAULT_CONNECT_ROWS: u32 = 30;
pub(crate) const MAX_CONNECT_DIMENSION: u32 = 1_000;
pub(super) const DEFAULT_SERIAL_BAUD_RATE: u32 = 9_600;
pub(super) const DEFAULT_SERIAL_DATA_BITS: u8 = 8;
pub(super) const DEFAULT_SERIAL_STOP_BITS: u8 = 1;

#[derive(Clone)]
pub struct ExternalControlRuntime {
    #[cfg_attr(not(test), allow(dead_code))]
    pub config: ExternalControlPermissions,
    #[cfg(test)]
    pub app_config: Option<AppConfig>,
    #[cfg(test)]
    pub available_serial_ports: Option<Vec<crate::serial::PortInfo>>,
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

    pub(super) fn ensure_connect_enabled(&self) -> Result<(), ExternalControlError> {
        if self.connect_enabled_now()? {
            Ok(())
        } else {
            Err(permission_denied(
                "New connections from external control are disabled. Set external_control.connect_enabled=true.",
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
                .map_err(|error| {
                    internal_error(format!("Failed to load the configuration: {error}"))
                })
        }
    }
}

#[cfg_attr(not(test), allow(unused_variables))]
pub(super) fn load_app_config(
    runtime: &ExternalControlRuntime,
) -> Result<AppConfig, ExternalControlError> {
    #[cfg(test)]
    {
        Ok(runtime.app_config.clone().unwrap_or_default())
    }

    #[cfg(not(test))]
    {
        config::config_read()
            .map_err(|error| internal_error(format!("Failed to load the configuration: {error}")))
    }
}

#[cfg_attr(not(test), allow(unused_variables))]
pub(super) fn load_serial_ports(
    runtime: &ExternalControlRuntime,
) -> Result<Vec<crate::serial::PortInfo>, ExternalControlError> {
    #[cfg(test)]
    {
        Ok(runtime.available_serial_ports.clone().unwrap_or_default())
    }

    #[cfg(not(test))]
    {
        crate::serial::list_ports().map_err(internal_error)
    }
}
