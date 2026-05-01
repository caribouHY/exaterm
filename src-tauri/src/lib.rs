mod ai;
mod config;
mod logger;
mod serial;
mod ssh;
mod ssh_known_hosts;
mod telnet;

use logger::LoggerState;
use serial::SerialState;
use ssh::SshState;
use telnet::TelnetState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(SshState::new())
        .manage(SerialState::new())
        .manage(TelnetState::new())
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
            logger::logger_get_sessions,
            logger::logger_get_log_dir,
            // Config
            config::config_load,
            config::config_save,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ExaTerm");
}
