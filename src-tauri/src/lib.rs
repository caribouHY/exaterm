mod ai;
mod app_exit;
mod app_update;
mod cli;
mod command_error;
mod config;
mod connect_attempt;
mod connection_history;
mod external_control;
mod logger;
mod mcp;
mod serial;
mod ssh;
mod ssh_known_hosts;
mod telnet;
mod terminal_cli;
mod terminal_control;
mod workspace;

use cli::{CliAction, StartupCliRequest, StartupCliState};
use connection_history::ConnectionHistoryState;
use external_control::{
    spawn_gui_control_plane, ExternalControlCredentialState, ExternalControlLogControlState,
    ExternalControlRuntime,
};
use logger::LoggerState;
use serial::SerialState;
use ssh::SshState;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use telnet::TelnetState;
use terminal_control::TerminalControlState;
use workspace::WorkspaceState;

pub use mcp::run_stdio_proxy;
pub use terminal_cli::run_terminal_cli;

#[tauri::command]
fn startup_cli_request_take(
    state: tauri::State<'_, StartupCliState>,
    window_id: String,
) -> Option<StartupCliRequest> {
    state.take_for_window(&window_id)
}

fn focus_window(app: &tauri::AppHandle, window_id: &str) {
    let Some(window) = app.get_webview_window(window_id) else {
        return;
    };
    if let Err(error) = window.show() {
        log::warn!("Startup CLI window show failed: {error}");
    }
    if let Err(error) = window.unminimize() {
        log::warn!("Startup CLI window unminimize failed: {error}");
    }
    if let Err(error) = window.set_focus() {
        log::warn!("Startup CLI window focus failed: {error}");
    }
}

fn handle_forwarded_cli_invocation(app: &tauri::AppHandle, args: Vec<String>) {
    let action = match cli::parse_forwarded_args(&args) {
        Ok(action) => action,
        Err(error) => {
            log::warn!("Forwarded CLI arguments were rejected: {}", error.message());
            return;
        }
    };
    let CliAction::RunApp(request) = action else {
        return;
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let window_id = app.state::<WorkspaceState>().preferred_window_id().await;
        if let Some(request) = request {
            app.state::<StartupCliState>()
                .enqueue(window_id.clone(), request);
            if let Err(error) = app.emit_to(&window_id, "startup-cli://request-available", ()) {
                log::warn!("Startup CLI request notification failed: {error}");
            }
        }
        focus_window(&app, &window_id);
    });
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
    let connection_history_state = ConnectionHistoryState::new();
    let external_control_credential_state = ExternalControlCredentialState::new();
    let external_control_log_control_state = ExternalControlLogControlState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_forwarded_cli_invocation(app, args);
        }))
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
        .manage(connection_history_state)
        .manage(external_control_credential_state.clone())
        .manage(external_control_log_control_state.clone())
        .on_window_event({
            let workspace_state = workspace_state.clone();
            let ssh_state = ssh_state.clone();
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
                    let ssh_state = ssh_state.clone();
                    let window_id = window.label().to_string();
                    tauri::async_runtime::spawn(async move {
                        ssh_state
                            .authentication_prompts
                            .cancel_window(&window_id)
                            .await;
                        ssh_state.host_key_prompts.cancel_window(&window_id).await;
                        let result = workspace_state.unregister_window(window_id).await;
                        workspace::emit_workspace_updates(&app, &result.snapshots);
                        workspace::emit_workspace_window_closed(&app, &result);
                        if result.remaining_window_count > 0 {
                            let destination_window_id = workspace_state.preferred_window_id().await;
                            let reassigned = app
                                .state::<StartupCliState>()
                                .reassign_window(&result.window_id, &destination_window_id);
                            if reassigned > 0 {
                                if let Err(error) = app.emit_to(
                                    &destination_window_id,
                                    "startup-cli://request-available",
                                    (),
                                ) {
                                    log::warn!(
                                        "Reassigned startup CLI request notification failed: {error}"
                                    );
                                }
                            }
                        }
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
                        let app_handle = app.handle().clone();
                        let io = external_control::service::ExternalControlIo::new(
                            Arc::new(
                                external_control::service::SystemExternalControlConfigIo,
                            ),
                            Arc::new(
                                external_control::service::TauriExternalControlProtocolIo::new(
                                    app_handle.clone(),
                                    terminal_control_state.clone(),
                                    workspace_state.clone(),
                                    ssh_state.clone(),
                                    serial_state.clone(),
                                    telnet_state.clone(),
                                    Some(logger_state.clone()),
                                ),
                            ),
                            Arc::new(external_control::service::TauriExternalControlUiIo::new(
                                app_handle,
                                Some(external_control_credential_state.clone()),
                                Some(external_control_log_control_state.clone()),
                            )),
                            Arc::new(external_control::service::LoggerExternalControlIo::new(
                                Some(logger_state.clone()),
                            )),
                        );
                        let runtime = ExternalControlRuntime {
                            io,
                            terminals: terminal_control_state.clone(),
                            workspace: workspace_state.clone(),
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
            startup_cli_request_take,
            ssh::ssh_algorithm_catalog,
            ssh::ssh_private_key_requires_passphrase,
            ssh::ssh_connect,
            ssh::ssh_connect_cancel,
            ssh::ssh_host_key_respond,
            ssh::ssh_authentication_respond,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_disconnect,
            // Serial
            serial::serial_list_ports,
            serial::serial_connect,
            serial::serial_connect_cancel,
            serial::serial_write,
            serial::serial_disconnect,
            // Telnet
            telnet::telnet_connect,
            telnet::telnet_connect_cancel,
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
            logger::logger_start_manual,
            logger::logger_start_on_connection,
            logger::logger_stop_manual,
            logger::logger_is_manual_active,
            logger::logger_append,
            logger::logger_get_sessions,
            logger::logger_bulk_delete_sessions,
            logger::logger_get_log_dir,
            // Connection history
            connection_history::connection_history_list,
            connection_history::connection_history_record,
            connection_history::connection_history_delete,
            connection_history::connection_history_clear,
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
            config::config_saved_connection_upsert,
            config::config_saved_connection_delete,
            // App updates
            app_update::app_update_check,
            app_update::app_update_install,
            // App lifecycle
            app_exit::app_exit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ExaTerm");
}
