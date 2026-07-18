mod auth;
mod client_config;
mod connection;
mod diagnostics;
mod host_key;
mod io;
mod jump;
mod profiles;
mod types;

#[cfg(test)]
mod tests;

pub use auth::private_key_requires_passphrase;
#[allow(unused_imports)]
pub use connection::connect;
#[cfg(not(test))]
pub use host_key::{verify_trusted_host_key, verify_trusted_host_key_via_jump};
pub use io::{write_data, SshState};
pub use profiles::resolve_jump_profile;
pub use types::{SshConnectOptions, SshConnectResult, SshJumpProfile, SshProbeHostKeyOptions};

pub(crate) use client_config::{legacy_algorithm_selection, validate_algorithm_config};

type SshCommandState<'a> = tauri::State<'a, SshState>;
type TerminalCommandState<'a> = tauri::State<'a, crate::terminal_control::TerminalControlState>;
type WorkspaceCommandState<'a> = tauri::State<'a, crate::workspace::WorkspaceState>;
type LoggerCommandState<'a> = tauri::State<'a, crate::logger::LoggerState>;
type LanguageCommandState<'a> = tauri::State<'a, crate::i18n::BackendLanguageState>;

fn localize<T>(
    language: &LanguageCommandState<'_>,
    result: Result<T, String>,
) -> Result<T, String> {
    result.map_err(|error| crate::i18n::translate_gui_error(language.inner(), &error))
}

#[tauri::command]
pub fn ssh_algorithm_catalog() -> client_config::SshAlgorithmCatalog {
    client_config::algorithm_catalog()
}

#[tauri::command]
pub fn ssh_private_key_requires_passphrase(
    language: LanguageCommandState<'_>,
    private_key_path: String,
) -> Result<bool, String> {
    localize(
        &language,
        auth::ssh_private_key_requires_passphrase(private_key_path),
    )
}

#[tauri::command]
pub async fn ssh_probe_host_key(
    app: tauri::AppHandle,
    state: SshCommandState<'_>,
    language: LanguageCommandState<'_>,
    options: SshProbeHostKeyOptions,
) -> Result<crate::ssh_known_hosts::HostKeyCheckResult, String> {
    localize(
        &language,
        host_key::ssh_probe_host_key(app, state, options).await,
    )
}

#[tauri::command]
pub async fn ssh_trust_host_key(
    state: SshCommandState<'_>,
    language: LanguageCommandState<'_>,
    host: String,
    port: u16,
    replace: bool,
) -> Result<(), String> {
    localize(
        &language,
        host_key::ssh_trust_host_key(state, host, port, replace).await,
    )
}

#[tauri::command]
pub async fn ssh_connect(
    app: tauri::AppHandle,
    state: SshCommandState<'_>,
    terminals: TerminalCommandState<'_>,
    workspace: WorkspaceCommandState<'_>,
    logger: LoggerCommandState<'_>,
    language: LanguageCommandState<'_>,
    options: SshConnectOptions,
) -> Result<SshConnectResult, String> {
    localize(
        &language,
        connection::connect(&app, &state, &terminals, &workspace, Some(&logger), options).await,
    )
}

#[tauri::command]
pub async fn ssh_write(
    state: SshCommandState<'_>,
    language: LanguageCommandState<'_>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    localize(&language, io::ssh_write(state, session_id, data).await)
}

#[tauri::command]
pub async fn ssh_resize(
    state: SshCommandState<'_>,
    language: LanguageCommandState<'_>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    localize(
        &language,
        io::ssh_resize(state, session_id, cols, rows).await,
    )
}

#[tauri::command]
pub async fn ssh_disconnect(
    app: tauri::AppHandle,
    state: SshCommandState<'_>,
    terminals: TerminalCommandState<'_>,
    workspace: WorkspaceCommandState<'_>,
    logger: LoggerCommandState<'_>,
    language: LanguageCommandState<'_>,
    session_id: String,
) -> Result<(), String> {
    localize(
        &language,
        io::ssh_disconnect(app, state, terminals, workspace, logger, session_id).await,
    )
}
