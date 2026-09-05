use serde_json::{json, Value};
#[cfg(not(test))]
use tauri::AppHandle;
#[cfg(test)]
use uuid::Uuid;

use crate::config::AppConfig;
#[cfg(not(test))]
use crate::external_control::protocol::ExternalControlCredentialState;
use crate::logger;
#[cfg(not(test))]
use crate::ssh;
#[cfg(not(test))]
use crate::telnet;
use crate::terminal_control::TerminalProtocol;
#[cfg(not(test))]
use crate::workspace::emit_workspace_updated;
use crate::workspace::{WorkspaceConnectionInfo, WorkspaceTabRegisterInput};

#[cfg(not(test))]
use super::profiles::ssh_credential_required;
use super::profiles::{
    prepare_direct_ssh_connection, prepare_direct_telnet_connection,
    prepare_saved_profile_connection, prepare_serial_console_connection,
};
use super::ExternalControlConnectionCreatedPayload;
#[cfg(not(test))]
use super::ExternalControlCredentialRequestPayload;
use super::PreparedConnectionKind;
#[cfg_attr(test, allow(unused_imports))]
use super::{
    internal_error, invalid_params, load_app_config, load_serial_ports, ConnectSavedProfileArgs,
    ConnectSerialConsoleArgs, ConnectSshArgs, ConnectTelnetArgs, ExternalControlError,
    ExternalControlRuntime, ExternalControlService, PreparedConnection, PreparedSerialConnection,
};

#[derive(Clone, Copy)]
enum ConnectionHostKeyHandling {
    RequireTrusted,
    PromptUnknown,
}

impl ExternalControlService {
    pub(crate) async fn connect_saved_profile(
        &self,
        args: ConnectSavedProfileArgs,
    ) -> Result<Value, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let config = load_app_config(&self.runtime)?;
        let prepared = prepare_saved_profile_connection(&config, args).map_err(invalid_params)?;
        connect_prepared_profile(
            &self.runtime,
            &config,
            prepared,
            ConnectionHostKeyHandling::RequireTrusted,
        )
        .await
    }

    pub(crate) async fn connect_ssh(
        &self,
        args: ConnectSshArgs,
    ) -> Result<Value, ExternalControlError> {
        self.ensure_direct_connect_enabled()?;
        let config = load_app_config(&self.runtime)?;
        let prepared = prepare_direct_ssh_connection(&config, args).map_err(invalid_params)?;
        connect_prepared_profile(
            &self.runtime,
            &config,
            prepared,
            ConnectionHostKeyHandling::PromptUnknown,
        )
        .await
    }

    pub(crate) async fn connect_telnet(
        &self,
        args: ConnectTelnetArgs,
    ) -> Result<Value, ExternalControlError> {
        self.ensure_direct_connect_enabled()?;
        let config = load_app_config(&self.runtime)?;
        let prepared = prepare_direct_telnet_connection(args).map_err(invalid_params)?;
        connect_prepared_profile(
            &self.runtime,
            &config,
            prepared,
            ConnectionHostKeyHandling::RequireTrusted,
        )
        .await
    }

    pub(crate) async fn list_serial_ports(&self) -> Result<Value, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let ports = load_serial_ports(&self.runtime)?;
        Ok(json!({
            "ports": ports,
        }))
    }

    pub(crate) async fn connect_serial_console(
        &self,
        args: ConnectSerialConsoleArgs,
    ) -> Result<Value, ExternalControlError> {
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

#[cfg(not(test))]
async fn connect_prepared_profile(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedConnection,
    host_key_handling: ConnectionHostKeyHandling,
) -> Result<Value, ExternalControlError> {
    let app = runtime.app.as_ref().ok_or_else(|| {
        internal_error("App handle required for external control connections is unavailable")
    })?;
    let session_id =
        connect_prepared_profile_session(runtime, app, config, &prepared, host_key_handling)
            .await?;
    let connection_info = workspace_connection_info(&prepared);

    finish_created_session(
        runtime,
        config,
        session_id,
        prepared.connection_type,
        prepared.target,
        prepared.title,
        prepared.encoding,
        prepared.terminal_mode,
        Some(connection_info),
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

#[cfg(not(test))]
async fn connect_prepared_profile_session(
    runtime: &ExternalControlRuntime,
    app: &AppHandle,
    config: &AppConfig,
    prepared: &PreparedConnection,
    host_key_handling: ConnectionHostKeyHandling,
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
                app,
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
            connect_prepared_telnet_profile(runtime, app, prepared, host, *port).await
        }
    }
}

#[cfg(not(test))]
struct PreparedSshProfileParts<'a> {
    host: &'a str,
    port: u16,
    username: &'a str,
    auth_method: &'a str,
    private_key_path: Option<&'a str>,
    jump_profile: Option<&'a ssh::SshJumpProfile>,
}

#[cfg(not(test))]
async fn connect_prepared_ssh_profile(
    runtime: &ExternalControlRuntime,
    app: &AppHandle,
    config: &AppConfig,
    prepared: &PreparedConnection,
    host_key_handling: ConnectionHostKeyHandling,
    parts: PreparedSshProfileParts<'_>,
) -> Result<String, ExternalControlError> {
    let credentials = runtime.credentials.as_ref().ok_or_else(|| {
        internal_error("Credential prompt state required for external control is unavailable")
    })?;
    let prompt_window_id = runtime.workspace.preferred_window_id().await;
    let jump_credential = request_jump_credential(credentials, app, parts.jump_profile).await?;
    let profile_credential = request_profile_credential(
        credentials,
        app,
        &prepared.profile_id,
        parts.host,
        parts.port,
        parts.username,
        parts.auth_method,
        parts.private_key_path,
        Some(&config.ssh.default_private_key_path),
        &prepared.target,
        &prepared.title,
    )
    .await?;

    let options = build_ssh_connect_options(prepared, parts, profile_credential, jump_credential);
    ssh::connect(
        app,
        &runtime.ssh,
        &runtime.terminals,
        &runtime.workspace,
        runtime.logger.as_ref(),
        prompt_window_id,
        match host_key_handling {
            ConnectionHostKeyHandling::RequireTrusted => ssh::HostKeyHandling::RequireTrusted,
            ConnectionHostKeyHandling::PromptUnknown => ssh::HostKeyHandling::PromptUnknown,
        },
        options,
        None,
    )
    .await
    .map_err(invalid_params)
    .map(|result| result.session_id)
}

#[cfg(not(test))]
async fn connect_prepared_telnet_profile(
    runtime: &ExternalControlRuntime,
    app: &AppHandle,
    prepared: &PreparedConnection,
    host: &str,
    port: u16,
) -> Result<String, ExternalControlError> {
    telnet::connect(
        app,
        &runtime.telnet,
        &runtime.terminals,
        &runtime.workspace,
        runtime.logger.as_ref(),
        host.to_string(),
        port,
        prepared.cols,
        prepared.rows,
        Some(prepared.encoding.clone()),
        None,
    )
    .await
    .map_err(invalid_params)
}

#[cfg(not(test))]
async fn request_jump_credential(
    credentials: &ExternalControlCredentialState,
    app: &AppHandle,
    jump_profile: Option<&ssh::SshJumpProfile>,
) -> Result<Option<String>, ExternalControlError> {
    let Some(jump_profile) = jump_profile else {
        return Ok(None);
    };
    request_profile_credential(
        credentials,
        app,
        &jump_profile.id,
        &jump_profile.host,
        jump_profile.port,
        &jump_profile.username,
        &jump_profile.auth_method,
        jump_profile.private_key_path.as_deref(),
        None,
        &format!(
            "{}@{}:{}",
            jump_profile.username, jump_profile.host, jump_profile.port
        ),
        &format!("{}@{}", jump_profile.username, jump_profile.host),
    )
    .await
}

#[cfg(not(test))]
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

#[cfg(not(test))]
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

#[cfg(not(test))]
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
    default_private_key_path: Option<&str>,
    target: &str,
    title: &str,
) -> Result<Option<String>, ExternalControlError> {
    if !ssh_credential_required(auth_method, private_key_path, default_private_key_path)
        .map_err(invalid_params)?
    {
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
        .ok_or_else(|| invalid_params("The external control credential prompt was cancelled"))
}

async fn finish_created_session(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    session_id: String,
    connection_type: String,
    target: String,
    title: String,
    encoding: String,
    terminal_mode: String,
    connection_info: Option<WorkspaceConnectionInfo>,
) -> Result<Value, ExternalControlError> {
    let auto_log_file_path = if config.terminal.auto_session_log {
        match &runtime.logger {
            Some(logger_state) => logger::start_log_on_connection(
                logger_state,
                session_id.clone(),
                connection_type.clone(),
                target.clone(),
            )
            .await
            .map(Some)
            .unwrap_or_else(|error| {
                log::warn!(
                    "External control connection log start failed for session {session_id}: {error}"
                );
                None
            }),
            None => {
                log::warn!(
                    "External control connection log start skipped because logger state is unavailable"
                );
                None
            }
        }
    } else {
        None
    };
    let auto_logging = auto_log_file_path.is_some();

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

    let _workspace_snapshot = runtime
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
    #[cfg(not(test))]
    {
        let app = runtime.app.as_ref().ok_or_else(|| {
            internal_error("App handle required for external control connections is unavailable")
        })?;
        emit_workspace_updated(app, &_workspace_snapshot);
    }

    Ok(json!(payload))
}

#[cfg(not(test))]
async fn connect_prepared_serial_console(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedSerialConnection,
) -> Result<Value, ExternalControlError> {
    let app = runtime.app.as_ref().ok_or_else(|| {
        internal_error("App handle required for external control connections is unavailable")
    })?;

    let session_id = crate::serial::connect(
        app,
        &runtime.serial,
        &runtime.terminals,
        &runtime.workspace,
        runtime.logger.as_ref(),
        prepared.port.clone(),
        prepared.config,
        Some(prepared.encoding.clone()),
        None,
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
        None,
    )
    .await
}

#[cfg(test)]
async fn connect_prepared_profile(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedConnection,
    _host_key_handling: ConnectionHostKeyHandling,
) -> Result<Value, ExternalControlError> {
    let session_id = Uuid::new_v4().to_string();
    let connection_info = workspace_connection_info(&prepared);
    let protocol =
        terminal_protocol_from_log_type(&prepared.connection_type).map_err(invalid_params)?;
    runtime
        .terminals
        .register_session_with_encoding(
            session_id.clone(),
            protocol,
            prepared.target.clone(),
            Some(prepared.encoding.clone()),
        )
        .await;
    finish_created_session(
        runtime,
        config,
        session_id,
        prepared.connection_type,
        prepared.target,
        prepared.title,
        prepared.encoding,
        prepared.terminal_mode,
        Some(connection_info),
    )
    .await
}

#[cfg(test)]
async fn connect_prepared_serial_console(
    runtime: &ExternalControlRuntime,
    config: &AppConfig,
    prepared: PreparedSerialConnection,
) -> Result<Value, ExternalControlError> {
    let session_id = Uuid::new_v4().to_string();
    runtime
        .terminals
        .register_session_with_encoding(
            session_id.clone(),
            TerminalProtocol::Serial,
            prepared.target.clone(),
            Some(prepared.encoding.clone()),
        )
        .await;
    finish_created_session(
        runtime,
        config,
        session_id,
        "serial".into(),
        prepared.target,
        prepared.title,
        prepared.encoding,
        prepared.terminal_mode,
        None,
    )
    .await
}
