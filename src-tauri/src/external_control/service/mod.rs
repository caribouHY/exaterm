use crate::config::AppConfig;
use crate::terminal_control::TerminalControlState;
use crate::workspace::WorkspaceState;

mod connections;
mod io;
mod profiles;
mod terminal;
mod types;

pub(crate) use io::*;

pub(crate) use profiles::normalize_direct_host;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use profiles::{
    available_serial_port_names, list_connection_profiles_from_config, normalize_connect_dimension,
    normalize_profile_auth_method, normalize_profile_encoding, normalize_profile_host,
    normalize_profile_string, normalize_profile_terminal_mode, normalize_profile_type,
    normalize_serial_data_bits, normalize_serial_stop_bits, prepare_direct_ssh_connection,
    prepare_direct_telnet_connection, prepare_saved_profile_connection,
    prepare_serial_console_connection, profile_external_control_enabled, ssh_credential_required,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use terminal::{normalize_max_chars, normalize_timeout_ms};
pub(super) use types::{internal_error, invalid_params, not_found, permission_denied, unavailable};
pub(crate) use types::{
    ConnectSavedProfileArgs, ConnectSerialConsoleArgs, ConnectSshArgs, ConnectTelnetArgs,
    ExternalControlConnectionProfile, ExternalControlEncoding, ExternalControlLogControlAck,
    ExternalControlLogWriteMode, ExternalControlSerialFlowControl, ExternalControlSerialParity,
    ExternalControlSshAuthMethod, ExternalControlTerminalMode, ListConnectionProfilesArgs,
    PreparedConnection, PreparedConnectionKind, PreparedSerialConnection, ReadTerminalOutputArgs,
    RunTerminalCommandArgs, SavedProfileConnectionType, SendTerminalInputArgs,
    StartTerminalLogArgs, StopTerminalLogArgs,
};
pub(crate) use types::{
    ConnectionCreatedResult, ListConnectionProfilesResult, ListSerialPortsResult,
    ReadTerminalOutputResult, RunTerminalCommandResult, SendTerminalInputResult,
    SetTerminalLogPausedResult, StartTerminalLogResult, StopTerminalLogResult, TerminalLogState,
    TerminalLogStatusResult, TerminalOutputResult, WaitTerminalOutputResult,
};
pub(crate) use types::{
    ExternalControlCredentialRequestPayload, ExternalControlLogControlRequestPayload,
};
pub use types::{
    ExternalControlError, ExternalControlRequest, ExternalControlResponse,
    ListTerminalSessionsResult, TerminalLogSessionArgs,
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
    pub(crate) io: ExternalControlIo,
    pub terminals: TerminalControlState,
    pub workspace: WorkspaceState,
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
                .map(ExternalControlResponse::ListTerminalSessions),
            ExternalControlRequest::ListConnectionProfiles(args) => self
                .list_connection_profiles(args)
                .await
                .map(ExternalControlResponse::ListConnectionProfiles),
            ExternalControlRequest::ConnectSavedProfile(args) => self
                .connect_saved_profile(args)
                .await
                .map(ExternalControlResponse::ConnectSavedProfile),
            ExternalControlRequest::ConnectSsh(args) => self
                .connect_ssh(args)
                .await
                .map(ExternalControlResponse::ConnectSsh),
            ExternalControlRequest::ConnectTelnet(args) => self
                .connect_telnet(args)
                .await
                .map(ExternalControlResponse::ConnectTelnet),
            ExternalControlRequest::ListSerialPorts => self
                .list_serial_ports()
                .await
                .map(ExternalControlResponse::ListSerialPorts),
            ExternalControlRequest::ConnectSerialConsole(args) => self
                .connect_serial_console(args)
                .await
                .map(ExternalControlResponse::ConnectSerialConsole),
            ExternalControlRequest::ReadTerminalOutput(args) => self
                .read_terminal_output(args)
                .await
                .map(ExternalControlResponse::ReadTerminalOutput),
            ExternalControlRequest::SendTerminalInput(args) => self
                .send_terminal_input(args)
                .await
                .map(ExternalControlResponse::SendTerminalInput),
            ExternalControlRequest::StartTerminalLog(args) => self
                .start_terminal_log(args)
                .await
                .map(ExternalControlResponse::StartTerminalLog),
            ExternalControlRequest::StopTerminalLog(args) => self
                .stop_terminal_log(args)
                .await
                .map(ExternalControlResponse::StopTerminalLog),
            ExternalControlRequest::GetTerminalLogStatus(args) => self
                .get_terminal_log_status(args)
                .await
                .map(ExternalControlResponse::GetTerminalLogStatus),
            ExternalControlRequest::PauseTerminalLog(args) => self
                .set_terminal_log_paused(args, true)
                .await
                .map(ExternalControlResponse::PauseTerminalLog),
            ExternalControlRequest::ResumeTerminalLog(args) => self
                .set_terminal_log_paused(args, false)
                .await
                .map(ExternalControlResponse::ResumeTerminalLog),
            ExternalControlRequest::RunTerminalCommand(args) => self
                .run_terminal_command(args)
                .await
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

    pub(super) fn ensure_direct_connect_enabled(&self) -> Result<(), ExternalControlError> {
        self.ensure_connect_enabled()?;
        if self.direct_connect_enabled_now()? {
            Ok(())
        } else {
            Err(permission_denied(
                "Direct SSH and Telnet connections from external control are disabled. Set external_control.direct_connect_enabled=true.",
            ))
        }
    }

    fn connect_enabled_now(&self) -> Result<bool, ExternalControlError> {
        load_app_config(&self.runtime).map(|config| config.external_control.connect_enabled)
    }

    fn direct_connect_enabled_now(&self) -> Result<bool, ExternalControlError> {
        load_app_config(&self.runtime).map(|config| config.external_control.direct_connect_enabled)
    }
}

pub(super) fn load_app_config(
    runtime: &ExternalControlRuntime,
) -> Result<AppConfig, ExternalControlError> {
    runtime
        .io
        .config
        .load_config()
        .map_err(|error| internal_error(format!("Failed to load the configuration: {error}")))
}

pub(super) fn load_serial_ports(
    runtime: &ExternalControlRuntime,
) -> Result<Vec<crate::serial::PortInfo>, ExternalControlError> {
    runtime
        .io
        .config
        .list_serial_ports()
        .map_err(internal_error)
}
