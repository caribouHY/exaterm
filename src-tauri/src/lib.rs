mod ai;
mod app_update;
mod cli;
mod config;
mod external_control;
mod i18n;
mod logger;
mod mcp;
mod serial;
mod ssh;
mod ssh_known_hosts;
mod telnet;
mod terminal_cli;
mod terminal_control;
mod workspace;

use cli::{CliAction, StartupCliRequest};
use external_control::{
    spawn_gui_control_plane, ExternalControlCredentialState, ExternalControlLogControlState,
    ExternalControlRuntime,
};
use i18n::BackendLanguageState;
use logger::LoggerState;
use serial::SerialState;
use ssh::SshState;
use std::sync::Mutex;
use tauri::Manager;
use telnet::TelnetState;
use terminal_control::TerminalControlState;
use workspace::WorkspaceState;

pub use mcp::run_stdio_proxy;
pub use terminal_cli::run_terminal_cli;

pub struct StartupCliState {
    request: Mutex<Option<StartupCliRequest>>,
}

impl StartupCliState {
    fn new(request: Option<StartupCliRequest>) -> Self {
        Self {
            request: Mutex::new(request),
        }
    }
}

#[tauri::command]
fn startup_cli_request_get(state: tauri::State<'_, StartupCliState>) -> Option<StartupCliRequest> {
    state.request.lock().ok()?.take()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let startup_cli_request = match cli::parse_env_args() {
        Ok(CliAction::RunApp(request)) => request,
        Ok(CliAction::PrintHelp) => {
            cli::print_help();
            std::process::exit(0);
        }
        Err(error) => {
            cli::print_error(&error);
            std::process::exit(1);
        }
    };

    let ssh_state = SshState::new();
    let serial_state = SerialState::new();
    let telnet_state = TelnetState::new();
    let terminal_control_state = TerminalControlState::new();
    let workspace_state = WorkspaceState::new();
    let logger_state = LoggerState::new();
    let external_control_credential_state = ExternalControlCredentialState::new();
    let external_control_log_control_state = ExternalControlLogControlState::new();
    let backend_language_state = BackendLanguageState::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_update::AppUpdateState::new())
        .manage(StartupCliState::new(startup_cli_request))
        .manage(ssh_state.clone())
        .manage(serial_state.clone())
        .manage(telnet_state.clone())
        .manage(terminal_control_state.clone())
        .manage(workspace_state.clone())
        .manage(logger_state.clone())
        .manage(external_control_credential_state.clone())
        .manage(external_control_log_control_state.clone())
        .manage(backend_language_state)
        .on_window_event({
            let workspace_state = workspace_state.clone();
            move |window, event| match event {
                tauri::WindowEvent::Focused(true) => {
                    let app = window.app_handle().clone();
                    let workspace_state = workspace_state.clone();
                    let window_id = window.label().to_string();
                    tauri::async_runtime::spawn(async move {
                        let snapshot = workspace_state.focus_window(window_id).await;
                        workspace::emit_workspace_updated(&app, &snapshot);
                    });
                }
                tauri::WindowEvent::Destroyed => {
                    let app = window.app_handle().clone();
                    let workspace_state = workspace_state.clone();
                    let window_id = window.label().to_string();
                    tauri::async_runtime::spawn(async move {
                        let result = workspace_state.unregister_window(window_id).await;
                        workspace::emit_workspace_updates(&app, &result.snapshots);
                        workspace::emit_workspace_window_closed(&app, &result);
                    });
                }
                _ => {}
            }
        })
        .setup(move |app| {
            #[cfg(test)]
            let _ = app;

            match config::config_load() {
                Ok(cfg) => {
                    terminal_control_state
                        .set_output_limit_from_scrollback(cfg.terminal.scrollback);
                    if cfg.external_control.enabled {
                        let runtime = ExternalControlRuntime {
                            config: external_control::service::ExternalControlPermissions::new(
                                cfg.external_control.connect_enabled,
                            ),
                            #[cfg(test)]
                            app_config: None,
                            #[cfg(test)]
                            available_serial_ports: None,
                            #[cfg(not(test))]
                            app: Some(app.handle().clone()),
                            terminals: terminal_control_state.clone(),
                            workspace: workspace_state.clone(),
                            ssh: ssh_state.clone(),
                            serial: serial_state.clone(),
                            telnet: telnet_state.clone(),
                            logger: Some(logger_state.clone()),
                            log_control: Some(external_control_log_control_state.clone()),
                            #[cfg(not(test))]
                            credentials: Some(external_control_credential_state.clone()),
                        };
                        spawn_gui_control_plane(runtime);
                    }
                }
                Err(error) => {
                    log::warn!(
                        "External control runtime not started because config could not be loaded: {error}"
                    );
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // SSH
            startup_cli_request_get,
            ssh::ssh_algorithm_catalog,
            ssh::ssh_probe_host_key,
            ssh::ssh_trust_host_key,
            ssh::ssh_private_key_requires_passphrase,
            ssh::ssh_connect,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_disconnect,
            // Serial
            serial::serial_list_ports,
            serial::serial_connect,
            serial::serial_write,
            serial::serial_disconnect,
            // Telnet
            telnet::telnet_connect,
            telnet::telnet_write,
            telnet::telnet_resize,
            telnet::telnet_disconnect,
            // AI
            ai::ai_get_models,
            ai::ai_get_ollama_models,
            ai::ai_secret_status,
            ai::ai_secret_set,
            ai::ai_secret_clear,
            ai::ai_chat,
            // Logger
            logger::logger_start,
            logger::logger_start_auto,
            logger::logger_start_manual,
            logger::logger_stop_manual,
            logger::logger_is_manual_active,
            logger::logger_append,
            logger::logger_append_to_mode,
            logger::logger_get_sessions,
            logger::logger_bulk_delete_sessions,
            logger::logger_get_log_dir,
            external_control::protocol::external_control_credential_submit,
            external_control::protocol::external_control_log_control_submit,
            terminal_control::terminal_encoding_set,
            terminal_control::terminal_output_delta_get,
            terminal_control::terminal_output_snapshot_get,
            workspace::commands::workspace_snapshot_get,
            workspace::commands::workspace_tab_activate,
            workspace::commands::workspace_tab_detach_to_new_window,
            workspace::commands::workspace_tab_drag_cancel,
            workspace::commands::workspace_tab_drag_drop,
            workspace::commands::workspace_tab_drag_hover,
            workspace::commands::workspace_tab_drag_start,
            workspace::commands::workspace_tab_drag_update,
            workspace::commands::workspace_tab_move,
            workspace::commands::workspace_tab_register,
            workspace::commands::workspace_tab_remove,
            workspace::commands::workspace_tab_reorder,
            workspace::commands::workspace_tab_update_metadata,
            workspace::commands::workspace_window_create,
            workspace::commands::workspace_window_focus,
            workspace::commands::workspace_window_register,
            workspace::commands::workspace_window_unregister,
            // Config
            config::config_load,
            config::config_save,
            i18n::backend_language_set,
            // App updates
            app_update::app_update_check,
            app_update::app_update_install,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ExaTerm");
}
