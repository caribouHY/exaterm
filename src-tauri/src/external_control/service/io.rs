use std::sync::Arc;

use async_trait::async_trait;
use tauri::AppHandle;

use crate::config::{self, AppConfig};
use crate::external_control::protocol::{
    ExternalControlCredentialState, ExternalControlLogControlState,
};
use crate::logger::{self, LoggerState};
use crate::serial::{self, PortInfo, SerialConfig, SerialState};
use crate::ssh::{self, SshConnectOptions, SshState};
use crate::telnet::{self, TelnetState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol};
use crate::workspace::{self, WorkspaceSnapshot, WorkspaceState};

use super::{
    ExternalControlCredentialRequestPayload, ExternalControlLogControlAck,
    ExternalControlLogControlRequestPayload,
};

pub(crate) trait ExternalControlConfigIo: Send + Sync {
    fn load_config(&self) -> Result<AppConfig, String>;
    fn list_serial_ports(&self) -> Result<Vec<PortInfo>, String>;
}

#[derive(Clone, Default)]
pub(crate) struct SystemExternalControlConfigIo;

impl ExternalControlConfigIo for SystemExternalControlConfigIo {
    fn load_config(&self) -> Result<AppConfig, String> {
        config::config_read()
    }

    fn list_serial_ports(&self) -> Result<Vec<PortInfo>, String> {
        serial::list_ports()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalControlHostKeyHandling {
    RequireTrusted,
    PromptUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalControlSshConnectRequest {
    pub(crate) prompt_window_id: String,
    pub(crate) host_key_handling: ExternalControlHostKeyHandling,
    pub(crate) options: SshConnectOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalControlTelnetConnectRequest {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) cols: u32,
    pub(crate) rows: u32,
    pub(crate) encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalControlSerialConnectRequest {
    pub(crate) port: String,
    pub(crate) config: SerialConfig,
    pub(crate) encoding: String,
}

#[async_trait]
pub(crate) trait ExternalControlProtocolIo: Send + Sync {
    async fn connect_ssh(
        &self,
        request: ExternalControlSshConnectRequest,
    ) -> Result<String, String>;
    async fn connect_telnet(
        &self,
        request: ExternalControlTelnetConnectRequest,
    ) -> Result<String, String>;
    async fn connect_serial(
        &self,
        request: ExternalControlSerialConnectRequest,
    ) -> Result<String, String>;
    async fn write_terminal(
        &self,
        protocol: TerminalProtocol,
        session_id: &str,
        data: String,
    ) -> Result<(), String>;
    async fn disconnect_terminal(
        &self,
        protocol: TerminalProtocol,
        session_id: &str,
    ) -> Result<(), String>;
}

#[derive(Clone)]
pub(crate) struct TauriExternalControlProtocolIo {
    app: AppHandle,
    terminals: TerminalControlState,
    workspace: WorkspaceState,
    ssh: SshState,
    serial: SerialState,
    telnet: TelnetState,
    logger: Option<LoggerState>,
}

impl TauriExternalControlProtocolIo {
    pub(crate) fn new(
        app: AppHandle,
        terminals: TerminalControlState,
        workspace: WorkspaceState,
        ssh: SshState,
        serial: SerialState,
        telnet: TelnetState,
        logger: Option<LoggerState>,
    ) -> Self {
        Self {
            app,
            terminals,
            workspace,
            ssh,
            serial,
            telnet,
            logger,
        }
    }
}

#[async_trait]
impl ExternalControlProtocolIo for TauriExternalControlProtocolIo {
    async fn connect_ssh(
        &self,
        request: ExternalControlSshConnectRequest,
    ) -> Result<String, String> {
        let host_key_handling = match request.host_key_handling {
            ExternalControlHostKeyHandling::RequireTrusted => ssh::HostKeyHandling::RequireTrusted,
            ExternalControlHostKeyHandling::PromptUnknown => ssh::HostKeyHandling::PromptUnknown,
        };
        Box::pin(ssh::connect(
            ssh::SshConnectRuntime {
                app: &self.app,
                state: &self.ssh,
                terminals: &self.terminals,
                workspace: &self.workspace,
                logger: self.logger.as_ref(),
            },
            ssh::SshConnectRequest {
                prompt_window_id: request.prompt_window_id,
                host_key_handling,
                options: request.options,
                attempt: None,
            },
        ))
        .await
        .map(|result| result.session_id)
    }

    async fn connect_telnet(
        &self,
        request: ExternalControlTelnetConnectRequest,
    ) -> Result<String, String> {
        Box::pin(telnet::connect(
            telnet::TelnetConnectRuntime {
                app: &self.app,
                state: &self.telnet,
                terminals: &self.terminals,
                workspace: &self.workspace,
                logger: self.logger.as_ref(),
            },
            telnet::TelnetConnectRequest {
                host: request.host,
                port: request.port,
                cols: request.cols,
                rows: request.rows,
                encoding: Some(request.encoding),
            },
            None,
        ))
        .await
    }

    async fn connect_serial(
        &self,
        request: ExternalControlSerialConnectRequest,
    ) -> Result<String, String> {
        Box::pin(serial::connect(
            serial::SerialConnectRuntime {
                app: &self.app,
                state: &self.serial,
                terminals: &self.terminals,
                workspace: &self.workspace,
                logger: self.logger.as_ref(),
            },
            serial::SerialConnectRequest {
                port: request.port,
                config: request.config,
                encoding: Some(request.encoding),
            },
            None,
        ))
        .await
    }

    async fn write_terminal(
        &self,
        protocol: TerminalProtocol,
        session_id: &str,
        data: String,
    ) -> Result<(), String> {
        match protocol {
            TerminalProtocol::Ssh => {
                ssh::write_data(&self.ssh, &self.terminals, session_id, data).await
            }
            TerminalProtocol::Serial => {
                serial::write_data(&self.serial, &self.terminals, session_id, data).await
            }
            TerminalProtocol::Telnet => {
                telnet::write_data(&self.telnet, &self.terminals, session_id, data).await
            }
        }
    }

    async fn disconnect_terminal(
        &self,
        protocol: TerminalProtocol,
        session_id: &str,
    ) -> Result<(), String> {
        match protocol {
            TerminalProtocol::Ssh => {
                ssh::disconnect(
                    &self.app,
                    &self.ssh,
                    &self.terminals,
                    &self.workspace,
                    self.logger.as_ref(),
                    session_id,
                )
                .await
            }
            TerminalProtocol::Serial => {
                serial::disconnect(
                    &self.app,
                    &self.serial,
                    &self.terminals,
                    &self.workspace,
                    self.logger.as_ref(),
                    session_id,
                )
                .await
            }
            TerminalProtocol::Telnet => {
                telnet::disconnect(
                    &self.app,
                    &self.telnet,
                    &self.terminals,
                    &self.workspace,
                    self.logger.as_ref(),
                    session_id,
                )
                .await
            }
        }
    }
}

#[async_trait]
pub(crate) trait ExternalControlUiIo: Send + Sync {
    fn private_key_requires_passphrase(&self, path: &str) -> Result<bool, String>;
    async fn request_ssh_credential(
        &self,
        payload: ExternalControlCredentialRequestPayload,
    ) -> Result<Option<String>, String>;
    async fn request_log_control(
        &self,
        window_id: &str,
        event: &str,
        payload: ExternalControlLogControlRequestPayload,
    ) -> Result<ExternalControlLogControlAck, String>;
    fn emit_workspace_updated(&self, snapshot: &WorkspaceSnapshot);
}

#[derive(Clone)]
pub(crate) struct TauriExternalControlUiIo {
    app: AppHandle,
    credentials: Option<ExternalControlCredentialState>,
    log_control: Option<ExternalControlLogControlState>,
}

impl TauriExternalControlUiIo {
    pub(crate) fn new(
        app: AppHandle,
        credentials: Option<ExternalControlCredentialState>,
        log_control: Option<ExternalControlLogControlState>,
    ) -> Self {
        Self {
            app,
            credentials,
            log_control,
        }
    }
}

#[async_trait]
impl ExternalControlUiIo for TauriExternalControlUiIo {
    fn private_key_requires_passphrase(&self, path: &str) -> Result<bool, String> {
        ssh::private_key_requires_passphrase(path)
    }

    async fn request_ssh_credential(
        &self,
        payload: ExternalControlCredentialRequestPayload,
    ) -> Result<Option<String>, String> {
        let credentials = self.credentials.as_ref().ok_or_else(|| {
            "Credential prompt state required for external control is unavailable".to_string()
        })?;
        credentials.request_ssh_credential(&self.app, payload).await
    }

    async fn request_log_control(
        &self,
        window_id: &str,
        event: &str,
        payload: ExternalControlLogControlRequestPayload,
    ) -> Result<ExternalControlLogControlAck, String> {
        let log_control = self.log_control.as_ref().ok_or_else(|| {
            "Log control state required for external control logging is unavailable".to_string()
        })?;
        log_control
            .request(&self.app, window_id, event, payload)
            .await
    }

    fn emit_workspace_updated(&self, snapshot: &WorkspaceSnapshot) {
        workspace::emit_workspace_updated(&self.app, snapshot);
    }
}

#[async_trait]
pub(crate) trait ExternalControlLogIo: Send + Sync {
    fn is_available(&self) -> bool;
    async fn active_log_session(&self, session_id: &str) -> Option<logger::LogSession>;
    async fn start_auto_log(
        &self,
        session_id: String,
        connection_type: String,
        target: String,
    ) -> Result<String, String>;
}

#[derive(Clone)]
pub(crate) struct LoggerExternalControlIo {
    state: Option<LoggerState>,
}

impl LoggerExternalControlIo {
    pub(crate) fn new(state: Option<LoggerState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl ExternalControlLogIo for LoggerExternalControlIo {
    fn is_available(&self) -> bool {
        self.state.is_some()
    }

    async fn active_log_session(&self, session_id: &str) -> Option<logger::LogSession> {
        let state = self.state.as_ref()?;
        logger::active_log_session(state, session_id).await
    }

    async fn start_auto_log(
        &self,
        session_id: String,
        connection_type: String,
        target: String,
    ) -> Result<String, String> {
        let state = self.state.as_ref().ok_or_else(|| {
            "Logger state required for external control is unavailable".to_string()
        })?;
        logger::start_log_on_connection(state, session_id, connection_type, target).await
    }
}

#[derive(Clone)]
pub(crate) struct ExternalControlIo {
    pub(crate) config: Arc<dyn ExternalControlConfigIo>,
    pub(crate) protocol: Arc<dyn ExternalControlProtocolIo>,
    pub(crate) ui: Arc<dyn ExternalControlUiIo>,
    pub(crate) logger: Arc<dyn ExternalControlLogIo>,
}

impl ExternalControlIo {
    pub(crate) fn new(
        config: Arc<dyn ExternalControlConfigIo>,
        protocol: Arc<dyn ExternalControlProtocolIo>,
        ui: Arc<dyn ExternalControlUiIo>,
        logger: Arc<dyn ExternalControlLogIo>,
    ) -> Self {
        Self {
            config,
            protocol,
            ui,
            logger,
        }
    }
}
