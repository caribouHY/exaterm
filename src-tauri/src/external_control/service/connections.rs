use crate::config::AppConfig;
use crate::ssh;
use crate::terminal_control::TerminalProtocol;
use crate::workspace::{WorkspaceConnectionInfo, WorkspaceTabRegisterInput};

use super::profiles::{
    prepare_direct_ssh_connection, prepare_direct_telnet_connection,
    prepare_saved_profile_connection, prepare_serial_console_connection,
    ssh_credential_required_with,
};
use super::PreparedConnectionKind;
use super::{
    invalid_params, load_app_config, load_serial_ports, ConnectSavedProfileArgs,
    ConnectSerialConsoleArgs, ConnectSshArgs, ConnectTelnetArgs, ConnectionCreatedResult,
    ExternalControlCredentialRequestPayload, ExternalControlError, ExternalControlHostKeyHandling,
    ExternalControlRuntime, ExternalControlSerialConnectRequest, ExternalControlService,
    ExternalControlSshConnectRequest, ExternalControlTelnetConnectRequest, ListSerialPortsResult,
    PreparedConnection, PreparedSerialConnection,
};

struct CreatedSessionMetadata {
    session_id: String,
    connection_type: String,
    target: String,
    title: String,
    encoding: String,
    terminal_mode: String,
    connection_info: Option<WorkspaceConnectionInfo>,
}

impl ExternalControlService {
    pub(crate) async fn connect_saved_profile(
        &self,
        args: ConnectSavedProfileArgs,
    ) -> Result<ConnectionCreatedResult, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let config = load_app_config(&self.runtime)?;
        let prepared = prepare_saved_profile_connection(&config, args).map_err(invalid_params)?;
        connect_prepared_profile(
            &self.runtime,
            &config,
            prepared,
            ExternalControlHostKeyHandling::RequireTrusted,
        )
        .await
    }

    pub(crate) async fn connect_ssh(
        &self,
        args: ConnectSshArgs,
    ) -> Result<ConnectionCreatedResult, ExternalControlError> {
        self.ensure_direct_connect_enabled()?;
        let config = load_app_config(&self.runtime)?;
        let prepared = prepare_direct_ssh_connection(&config, args).map_err(invalid_params)?;
        connect_prepared_profile(
            &self.runtime,
            &config,
            prepared,
            ExternalControlHostKeyHandling::PromptUnknown,
        )
        .await
    }

    pub(crate) async fn connect_telnet(
        &self,
        args: ConnectTelnetArgs,
    ) -> Result<ConnectionCreatedResult, ExternalControlError> {
        self.ensure_direct_connect_enabled()?;
        let config = load_app_config(&self.runtime)?;
        let prepared = prepare_direct_telnet_connection(args).map_err(invalid_params)?;
        connect_prepared_profile(
            &self.runtime,
            &config,
            prepared,
            ExternalControlHostKeyHandling::RequireTrusted,
        )
        .await
    }

    pub(crate) async fn list_serial_ports(
        &self,
    ) -> Result<ListSerialPortsResult, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let ports = load_serial_ports(&self.runtime)?;
        Ok(ListSerialPortsResult { ports })
    }

    pub(crate) async fn connect_serial_console(
        &self,
        args: ConnectSerialConsoleArgs,
    ) -> Result<ConnectionCreatedResult, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let ports = load_serial_ports(&self.runtime)?;
        let prepared = prepare_serial_console_connection(args, &ports).map_err(invalid_params)?;
        let config = load_app_config(&self.runtime)?;
        connect_prepared_serial_console(&self.runtime, &config, prepared).await
    }
}

pub(super) fn terminal_protocol_log_type(protocol: TerminalProtocol) -> &'static str {
    match protocol {
        TerminalProtocol::Ssh => "ssh",
        TerminalProtocol::Serial => "serial",
        TerminalProtocol::Telnet => "telnet",
    }
}

fn terminal_protocol_from_log_type(value: &str) -> Result<TerminalProtocol, String> {
    match value {
        "ssh" => Ok(TerminalProtocol::Ssh),
        "serial" => Ok(TerminalProtocol::Serial),
        "telnet" => Ok(TerminalProtocol::Telnet),
        _ => Err(format!("Unknown connection type: {value}")),
    }
}

async fn connect_prepared_profile(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedConnection,
    host_key_handling: ExternalControlHostKeyHandling,
) -> Result<ConnectionCreatedResult, ExternalControlError> {
    let session_id =
        connect_prepared_profile_session(runtime, config, &prepared, host_key_handling).await?;
    let connection_info = workspace_connection_info(&prepared);

    finish_created_session(
        runtime,
        config,
        CreatedSessionMetadata {
            session_id,
            connection_type: prepared.connection_type,
            target: prepared.target,
            title: prepared.title,
            encoding: prepared.encoding,
            terminal_mode: prepared.terminal_mode,
            connection_info: Some(connection_info),
        },
    )
    .await
}

fn workspace_connection_info(prepared: &PreparedConnection) -> WorkspaceConnectionInfo {
    match &prepared.kind {
        PreparedConnectionKind::Ssh {
            host,
            port,
            username,
            auth_method,
            private_key_path,
            jump_profile,
        } => WorkspaceConnectionInfo::Ssh {
            host: host.clone(),
            port: *port,
            username: username.clone(),
            auth_method: auth_method.clone(),
            private_key_path: private_key_path.clone(),
            jump_profile_id: jump_profile.as_ref().map(|profile| profile.id.clone()),
        },
        PreparedConnectionKind::Telnet { host, port } => WorkspaceConnectionInfo::Telnet {
            host: host.clone(),
            port: *port,
        },
    }
}

async fn connect_prepared_profile_session(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: &PreparedConnection,
    host_key_handling: ExternalControlHostKeyHandling,
) -> Result<String, ExternalControlError> {
    match &prepared.kind {
        PreparedConnectionKind::Ssh {
            host,
            port,
            username,
            auth_method,
            private_key_path,
            jump_profile,
        } => {
            connect_prepared_ssh_profile(
                runtime,
                config,
                prepared,
                host_key_handling,
                PreparedSshProfileParts {
                    host,
                    port: *port,
                    username,
                    auth_method,
                    private_key_path: private_key_path.as_deref(),
                    jump_profile: jump_profile.as_ref(),
                },
            )
            .await
        }
        PreparedConnectionKind::Telnet { host, port } => {
            connect_prepared_telnet_profile(runtime, prepared, host, *port).await
        }
    }
}

struct PreparedSshProfileParts<'a> {
    host: &'a str,
    port: u16,
    username: &'a str,
    auth_method: &'a str,
    private_key_path: Option<&'a str>,
    jump_profile: Option<&'a ssh::SshJumpProfile>,
}

struct ProfileCredentialRequest<'a> {
    payload: ExternalControlCredentialRequestPayload,
    private_key_path: Option<&'a str>,
    default_private_key_path: Option<&'a str>,
}

async fn connect_prepared_ssh_profile(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: &PreparedConnection,
    host_key_handling: ExternalControlHostKeyHandling,
    parts: PreparedSshProfileParts<'_>,
) -> Result<String, ExternalControlError> {
    let prompt_window_id = runtime.workspace.preferred_window_id().await;
    let jump_credential = request_jump_credential(runtime, parts.jump_profile).await?;
    let profile_credential = request_profile_credential(
        runtime,
        ProfileCredentialRequest {
            payload: ExternalControlCredentialRequestPayload {
                request_id: String::new(),
                profile_id: prepared.profile_id.clone(),
                host: parts.host.to_string(),
                port: parts.port,
                username: parts.username.to_string(),
                auth_method: parts.auth_method.to_string(),
                target: prepared.target.clone(),
                title: prepared.title.clone(),
            },
            private_key_path: parts.private_key_path,
            default_private_key_path: Some(&config.ssh.default_private_key_path),
        },
    )
    .await?;

    let options = build_ssh_connect_options(prepared, parts, profile_credential, jump_credential);
    runtime
        .io
        .protocol
        .connect_ssh(ExternalControlSshConnectRequest {
            prompt_window_id,
            host_key_handling,
            options,
        })
        .await
        .map_err(invalid_params)
}

async fn connect_prepared_telnet_profile(
    runtime: &ExternalControlRuntime,
    prepared: &PreparedConnection,
    host: &str,
    port: u16,
) -> Result<String, ExternalControlError> {
    runtime
        .io
        .protocol
        .connect_telnet(ExternalControlTelnetConnectRequest {
            host: host.to_string(),
            port,
            cols: prepared.cols,
            rows: prepared.rows,
            encoding: prepared.encoding.clone(),
        })
        .await
        .map_err(invalid_params)
}

async fn request_jump_credential(
    runtime: &ExternalControlRuntime,
    jump_profile: Option<&ssh::SshJumpProfile>,
) -> Result<Option<String>, ExternalControlError> {
    let Some(jump_profile) = jump_profile else {
        return Ok(None);
    };
    request_profile_credential(
        runtime,
        ProfileCredentialRequest {
            payload: ExternalControlCredentialRequestPayload {
                request_id: String::new(),
                profile_id: jump_profile.id.clone(),
                host: jump_profile.host.clone(),
                port: jump_profile.port,
                username: jump_profile.username.clone(),
                auth_method: jump_profile.auth_method.clone(),
                target: format!(
                    "{}@{}:{}",
                    jump_profile.username, jump_profile.host, jump_profile.port
                ),
                title: format!("{}@{}", jump_profile.username, jump_profile.host),
            },
            private_key_path: jump_profile.private_key_path.as_deref(),
            default_private_key_path: None,
        },
    )
    .await
}

fn build_ssh_connect_options(
    prepared: &PreparedConnection,
    parts: PreparedSshProfileParts<'_>,
    profile_credential: Option<String>,
    jump_credential: Option<String>,
) -> ssh::SshConnectOptions {
    let (password, key_passphrase) =
        split_required_ssh_credential(parts.auth_method, profile_credential);
    let (jump_password, jump_key_passphrase) = match parts.jump_profile {
        Some(jump_profile) => {
            split_optional_ssh_credential(&jump_profile.auth_method, jump_credential)
        }
        None => (None, None),
    };

    ssh::SshConnectOptions {
        host: parts.host.to_string(),
        port: parts.port,
        username: parts.username.to_string(),
        password,
        auth_method: Some(parts.auth_method.to_string()),
        private_key_path: parts.private_key_path.map(ToOwned::to_owned),
        key_passphrase,
        jump_profile_id: parts.jump_profile.map(|profile| profile.id.clone()),
        jump_password,
        jump_key_passphrase,
        cols: prepared.cols,
        rows: prepared.rows,
        encoding: Some(prepared.encoding.clone()),
        request_id: None,
    }
}

fn split_required_ssh_credential(
    auth_method: &str,
    credential: Option<String>,
) -> (String, Option<String>) {
    if auth_method == "password" {
        (credential.unwrap_or_default(), None)
    } else {
        (String::new(), credential)
    }
}

fn split_optional_ssh_credential(
    auth_method: &str,
    credential: Option<String>,
) -> (Option<String>, Option<String>) {
    if auth_method == "password" {
        (credential, None)
    } else {
        (None, credential)
    }
}

async fn request_profile_credential(
    runtime: &ExternalControlRuntime,
    request: ProfileCredentialRequest<'_>,
) -> Result<Option<String>, ExternalControlError> {
    if !ssh_credential_required_with(
        &request.payload.auth_method,
        request.private_key_path,
        request.default_private_key_path,
        |path| runtime.io.ui.private_key_requires_passphrase(path),
    )
    .map_err(invalid_params)?
    {
        return Ok(None);
    }

    runtime
        .io
        .ui
        .request_ssh_credential(request.payload)
        .await
        .map_err(invalid_params)?
        .map(Some)
        .ok_or_else(|| invalid_params("The external control credential prompt was cancelled"))
}

async fn finish_created_session(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    metadata: CreatedSessionMetadata,
) -> Result<ConnectionCreatedResult, ExternalControlError> {
    let CreatedSessionMetadata {
        session_id,
        connection_type,
        target,
        title,
        encoding,
        terminal_mode,
        connection_info,
    } = metadata;
    let auto_log_file_path = if config.terminal.auto_session_log {
        match runtime
            .io
            .logger
            .start_auto_log(session_id.clone(), connection_type.clone(), target.clone())
            .await
        {
            Ok(file_path) => Some(file_path),
            Err(error) => {
                log::warn!(
                    "External control connection log start failed for session {session_id}: {error}"
                );
                None
            }
        }
    } else {
        None
    };
    let auto_logging = auto_log_file_path.is_some();

    let protocol = terminal_protocol_from_log_type(&connection_type).map_err(invalid_params)?;
    let result = ConnectionCreatedResult {
        session_id: session_id.clone(),
        connection_type: connection_type.clone(),
        target,
        title: title.clone(),
        encoding: encoding.clone(),
        terminal_mode: terminal_mode.clone(),
        auto_logging,
    };

    let workspace_snapshot = runtime
        .workspace
        .register_tab(WorkspaceTabRegisterInput {
            window_id: None,
            tab_id: None,
            session_id,
            connection_type: protocol,
            title,
            encoding,
            terminal_mode,
            connection_info,
            is_manual_logging: auto_logging,
            manual_log_file_path: auto_log_file_path,
        })
        .await;
    runtime.io.ui.emit_workspace_updated(&workspace_snapshot);

    Ok(result)
}

async fn connect_prepared_serial_console(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedSerialConnection,
) -> Result<ConnectionCreatedResult, ExternalControlError> {
    let session_id = runtime
        .io
        .protocol
        .connect_serial(ExternalControlSerialConnectRequest {
            port: prepared.port.clone(),
            config: prepared.config,
            encoding: prepared.encoding.clone(),
        })
        .await
        .map_err(invalid_params)?;

    finish_created_session(
        runtime,
        config,
        CreatedSessionMetadata {
            session_id,
            connection_type: "serial".into(),
            target: prepared.target,
            title: prepared.title,
            encoding: prepared.encoding,
            terminal_mode: prepared.terminal_mode,
            connection_info: None,
        },
    )
    .await
}
