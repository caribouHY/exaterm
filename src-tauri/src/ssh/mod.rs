mod auth;
mod authentication_prompt;
mod client_config;
mod connection;
mod diagnostics;
mod host_key;
mod host_key_prompt;
mod io;
mod jump;
mod profiles;
mod types;

#[cfg(test)]
mod tests;

pub use auth::private_key_requires_passphrase;
#[allow(unused_imports)]
pub use connection::connect;
pub use host_key::HostKeyHandling;
pub use io::{write_data, SshState};
pub use profiles::resolve_jump_profile;
pub use types::{SshConnectOptions, SshConnectResult, SshJumpProfile};

pub(crate) use client_config::{legacy_algorithm_selection, validate_algorithm_config};

type SshCommandState<'a> = tauri::State<'a, SshState>;
type TerminalCommandState<'a> = tauri::State<'a, crate::terminal_control::TerminalControlState>;
type WorkspaceCommandState<'a> = tauri::State<'a, crate::workspace::WorkspaceState>;
type LoggerCommandState<'a> = tauri::State<'a, crate::logger::LoggerState>;

fn command_result<T>(
    result: Result<T, String>,
) -> Result<T, crate::command_error::BackendCommandError> {
    result.map_err(Into::into)
}

#[tauri::command]
pub fn ssh_algorithm_catalog() -> client_config::SshAlgorithmCatalog {
    client_config::algorithm_catalog()
}

#[tauri::command]
pub fn ssh_private_key_requires_passphrase(
    private_key_path: String,
) -> Result<bool, crate::command_error::BackendCommandError> {
    command_result(auth::ssh_private_key_requires_passphrase(private_key_path))
}

#[tauri::command]
pub async fn ssh_connect(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: SshCommandState<'_>,
    terminals: TerminalCommandState<'_>,
    workspace: WorkspaceCommandState<'_>,
    logger: LoggerCommandState<'_>,
    options: SshConnectOptions,
) -> Result<SshConnectResult, crate::command_error::BackendCommandError> {
    let request_id = options
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::command_error::BackendCommandError::new(
                "ssh.connect_request_id_required",
                "An SSH connection request ID is required",
            )
        })?
        .to_string();
    let attempt = state
        .connect_attempts
        .register(request_id)
        .map_err(crate::command_error::BackendCommandError::from)?;
    command_result(
        connection::connect(
            &app,
            &state,
            &terminals,
            &workspace,
            Some(&logger),
            window.label().to_string(),
            HostKeyHandling::Prompt,
            options,
            Some(attempt),
        )
        .await,
    )
}

#[tauri::command]
pub async fn ssh_connect_cancel(
    state: SshCommandState<'_>,
    request_id: String,
) -> Result<bool, crate::command_error::BackendCommandError> {
    let cancelled = state.connect_attempts.cancel(&request_id);
    if cancelled {
        state
            .authentication_prompts
            .cancel_connect_attempt(&request_id)
            .await;
        state
            .host_key_prompts
            .cancel_connect_attempt(&request_id)
            .await;
    }
    Ok(cancelled)
}

#[tauri::command]
pub async fn ssh_host_key_respond(
    state: SshCommandState<'_>,
    request_id: String,
    accept: bool,
) -> Result<(), crate::command_error::BackendCommandError> {
    command_result(state.host_key_prompts.submit(request_id, accept).await)
}

#[tauri::command]
pub async fn ssh_authentication_respond(
    state: SshCommandState<'_>,
    request_id: String,
    responses: Option<Vec<String>>,
) -> Result<(), crate::command_error::BackendCommandError> {
    command_result(
        state
            .authentication_prompts
            .submit(request_id, responses)
            .await,
    )
}

#[tauri::command]
pub async fn ssh_write(
    state: SshCommandState<'_>,
    terminals: TerminalCommandState<'_>,
    session_id: String,
    data: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    command_result(io::ssh_write(state, terminals, session_id, data).await)
}

#[tauri::command]
pub async fn ssh_resize(
    state: SshCommandState<'_>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), crate::command_error::BackendCommandError> {
    command_result(io::ssh_resize(state, session_id, cols, rows).await)
}

#[tauri::command]
pub async fn ssh_disconnect(
    app: tauri::AppHandle,
    state: SshCommandState<'_>,
    terminals: TerminalCommandState<'_>,
    workspace: WorkspaceCommandState<'_>,
    logger: LoggerCommandState<'_>,
    session_id: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    command_result(io::ssh_disconnect(app, state, terminals, workspace, logger, session_id).await)
}
