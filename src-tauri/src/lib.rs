mod ssh;
mod ssh_known_hosts;
mod serial;
mod ai;
mod logger;
mod config;

use ssh::SshState;
use serial::SerialState;
use logger::LoggerState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SshState::new())
        .manage(SerialState::new())
        .manage(LoggerState::new())
        .invoke_handler(tauri::generate_handler![
            // SSH
            ssh::ssh_probe_host_key,
            ssh::ssh_trust_host_key,
            ssh::ssh_connect,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_disconnect,
            // Serial
            serial::serial_list_ports,
            serial::serial_connect,
            serial::serial_write,
            serial::serial_disconnect,
            // AI
            ai::ai_get_models,
            ai::ai_get_ollama_models,
            ai::ai_secret_status,
            ai::ai_secret_set,
            ai::ai_secret_clear,
            ai::ai_chat,
            // Logger
            logger::logger_start,
            logger::logger_append,
            logger::logger_get_sessions,
            logger::logger_get_log_dir,
            // Config
            config::config_load,
            config::config_save,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ExaTerm");
}
