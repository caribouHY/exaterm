use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::workspace::WorkspaceConnectionInfo;

const CONNECTION_HISTORY_VERSION: u32 = 1;
const MAX_ENTRIES_PER_KIND: usize = 10;
pub const CONNECTION_HISTORY_UPDATED_EVENT: &str = "connection-history://updated";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionHistoryEntry {
    pub id: String,
    pub connection_info: WorkspaceConnectionInfo,
    pub encoding: String,
    pub terminal_mode: String,
    pub last_connected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ConnectionHistoryRecordInput {
    pub connection_info: WorkspaceConnectionInfo,
    pub encoding: String,
    pub terminal_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ConnectionHistoryFile {
    version: u32,
    entries: Vec<ConnectionHistoryEntry>,
}

impl Default for ConnectionHistoryFile {
    fn default() -> Self {
        Self {
            version: CONNECTION_HISTORY_VERSION,
            entries: Vec::new(),
        }
    }
}

pub struct ConnectionHistoryState {
    path: PathBuf,
    file_lock: Mutex<()>,
}

impl ConnectionHistoryState {
    pub fn new() -> Self {
        Self::with_path(connection_history_path())
    }

    fn with_path(path: PathBuf) -> Self {
        Self {
            path,
            file_lock: Mutex::new(()),
        }
    }
}

fn connection_history_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join("connection_history.json")
}

fn read_history(path: &Path) -> Result<ConnectionHistoryFile, String> {
    if !path.exists() {
        return Ok(ConnectionHistoryFile::default());
    }

    let data = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read connection history: {error}"))?;
    let mut history: ConnectionHistoryFile = serde_json::from_str(&data)
        .map_err(|error| format!("Failed to parse connection history: {error}"))?;
    if history.version != CONNECTION_HISTORY_VERSION {
        return Err(format!(
            "Unsupported connection history version: {}",
            history.version
        ));
    }
    for entry in &mut history.entries {
        if entry.id.trim().is_empty() {
            return Err("Connection history entry is missing an ID".into());
        }
        let normalized = normalize_input(ConnectionHistoryRecordInput {
            connection_info: entry.connection_info.clone(),
            encoding: entry.encoding.clone(),
            terminal_mode: entry.terminal_mode.clone(),
        })?;
        entry.connection_info = normalized.connection_info;
        entry.encoding = normalized.encoding;
        entry.terminal_mode = normalized.terminal_mode;
    }
    sort_and_limit(&mut history.entries);
    Ok(history)
}

fn write_history(path: &Path, history: &ConnectionHistoryFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create connection history directory: {error}"))?;
    }
    let data = serde_json::to_string_pretty(history)
        .map_err(|error| format!("Failed to serialize connection history: {error}"))?;
    fs::write(path, data).map_err(|error| format!("Failed to save connection history: {error}"))
}

fn clear_history_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to clear connection history: {error}")),
    }
}

fn normalize_input(
    mut input: ConnectionHistoryRecordInput,
) -> Result<ConnectionHistoryRecordInput, String> {
    if !matches!(input.encoding.as_str(), "utf-8" | "shift-jis" | "euc-jp") {
        return Err("Invalid connection history encoding".into());
    }
    if !matches!(
        input.terminal_mode.as_str(),
        "general"
            | "cisco_ios"
            | "arista_eos"
            | "juniper_junos"
            | "vyos"
            | "fujitsu_sir"
            | "allied_telesis_awplus"
            | "furukawa_fitelnet"
    ) {
        return Err("Invalid connection history terminal mode".into());
    }

    match &mut input.connection_info {
        WorkspaceConnectionInfo::Ssh {
            host,
            username,
            auth_method,
            private_key_path,
            jump_profile_id,
            ..
        } => {
            *host = host.trim().to_string();
            *username = username.trim().to_string();
            if host.is_empty() || username.is_empty() {
                return Err("SSH connection history requires a host and username".into());
            }
            if !matches!(
                auth_method.as_str(),
                "auto" | "password" | "keyboard_interactive" | "public_key"
            ) {
                return Err("Invalid SSH connection history authentication method".into());
            }
            *private_key_path = private_key_path
                .take()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            *jump_profile_id = jump_profile_id
                .take()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
        WorkspaceConnectionInfo::Telnet { host, .. } => {
            *host = host.trim().to_string();
            if host.is_empty() {
                return Err("Telnet connection history requires a host".into());
            }
        }
    }

    Ok(input)
}

fn same_destination(left: &WorkspaceConnectionInfo, right: &WorkspaceConnectionInfo) -> bool {
    match (left, right) {
        (
            WorkspaceConnectionInfo::Ssh {
                host: left_host,
                port: left_port,
                username: left_username,
                ..
            },
            WorkspaceConnectionInfo::Ssh {
                host: right_host,
                port: right_port,
                username: right_username,
                ..
            },
        ) => {
            left_host.eq_ignore_ascii_case(right_host)
                && left_port == right_port
                && left_username == right_username
        }
        (
            WorkspaceConnectionInfo::Telnet {
                host: left_host,
                port: left_port,
            },
            WorkspaceConnectionInfo::Telnet {
                host: right_host,
                port: right_port,
            },
        ) => left_host.eq_ignore_ascii_case(right_host) && left_port == right_port,
        _ => false,
    }
}

fn is_ssh(entry: &ConnectionHistoryEntry) -> bool {
    matches!(&entry.connection_info, WorkspaceConnectionInfo::Ssh { .. })
}

fn sort_and_limit(entries: &mut Vec<ConnectionHistoryEntry>) {
    entries.sort_by(|left, right| right.last_connected_at.cmp(&left.last_connected_at));
    let mut ssh_count = 0;
    let mut telnet_count = 0;
    entries.retain(|entry| {
        let count = if is_ssh(entry) {
            &mut ssh_count
        } else {
            &mut telnet_count
        };
        *count += 1;
        *count <= MAX_ENTRIES_PER_KIND
    });
}

fn upsert_history(
    history: &mut ConnectionHistoryFile,
    input: ConnectionHistoryRecordInput,
    connected_at: DateTime<Utc>,
) -> Result<(), String> {
    let input = normalize_input(input)?;
    let existing_id = history
        .entries
        .iter()
        .find(|entry| same_destination(&entry.connection_info, &input.connection_info))
        .map(|entry| entry.id.clone());
    history
        .entries
        .retain(|entry| !same_destination(&entry.connection_info, &input.connection_info));
    history.entries.push(ConnectionHistoryEntry {
        id: existing_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
        connection_info: input.connection_info,
        encoding: input.encoding,
        terminal_mode: input.terminal_mode,
        last_connected_at: connected_at,
    });
    sort_and_limit(&mut history.entries);
    Ok(())
}

fn remove_history_entry(history: &mut ConnectionHistoryFile, entry_id: &str) -> bool {
    let previous_len = history.entries.len();
    history.entries.retain(|entry| entry.id != entry_id);
    history.entries.len() != previous_len
}

#[tauri::command]
pub fn connection_history_list(
    state: tauri::State<'_, ConnectionHistoryState>,
) -> Result<Vec<ConnectionHistoryEntry>, crate::command_error::BackendCommandError> {
    let _guard = state
        .file_lock
        .lock()
        .map_err(|_| "Connection history lock is unavailable".to_string())?;
    Ok(read_history(&state.path)?.entries)
}

#[tauri::command]
pub fn connection_history_record(
    app: AppHandle,
    state: tauri::State<'_, ConnectionHistoryState>,
    input: ConnectionHistoryRecordInput,
) -> Result<(), crate::command_error::BackendCommandError> {
    if !crate::config::config_read()?.connection_history.enabled {
        return Ok(());
    }

    let guard = state
        .file_lock
        .lock()
        .map_err(|_| "Connection history lock is unavailable".to_string())?;
    let mut history = read_history(&state.path)?;
    upsert_history(&mut history, input, Utc::now())?;
    write_history(&state.path, &history)?;
    drop(guard);
    notify_history_updated(&app);
    Ok(())
}

#[tauri::command]
pub fn connection_history_delete(
    app: AppHandle,
    state: tauri::State<'_, ConnectionHistoryState>,
    entry_id: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    let guard = state
        .file_lock
        .lock()
        .map_err(|_| "Connection history lock is unavailable".to_string())?;
    let mut history = read_history(&state.path)?;
    if !remove_history_entry(&mut history, &entry_id) {
        return Ok(());
    }
    write_history(&state.path, &history)?;
    drop(guard);
    notify_history_updated(&app);
    Ok(())
}

#[tauri::command]
pub fn connection_history_clear(
    app: AppHandle,
    state: tauri::State<'_, ConnectionHistoryState>,
) -> Result<(), crate::command_error::BackendCommandError> {
    let guard = state
        .file_lock
        .lock()
        .map_err(|_| "Connection history lock is unavailable".to_string())?;
    clear_history_file(&state.path)?;
    drop(guard);
    notify_history_updated(&app);
    Ok(())
}

fn notify_history_updated(app: &AppHandle) {
    if let Err(error) = app.emit(CONNECTION_HISTORY_UPDATED_EVENT, ()) {
        log::warn!("Failed to notify connection history update: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn temp_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "exaterm-connection-history-{}.json",
            Uuid::new_v4()
        ))
    }

    fn ssh_input(host: &str, username: &str) -> ConnectionHistoryRecordInput {
        ConnectionHistoryRecordInput {
            connection_info: WorkspaceConnectionInfo::Ssh {
                host: host.into(),
                port: 22,
                username: username.into(),
                auth_method: "public_key".into(),
                private_key_path: Some(r"C:\Users\user\.ssh\id_ed25519".into()),
                jump_profile_id: Some("bastion".into()),
            },
            encoding: "utf-8".into(),
            terminal_mode: "general".into(),
        }
    }

    #[test]
    fn keyboard_interactive_ssh_history_is_valid() {
        let mut input = ssh_input("router.example", "operator");
        if let WorkspaceConnectionInfo::Ssh {
            auth_method,
            private_key_path,
            ..
        } = &mut input.connection_info
        {
            *auth_method = "keyboard_interactive".into();
            *private_key_path = None;
        }

        let normalized = normalize_input(input).unwrap();
        assert!(matches!(
            normalized.connection_info,
            WorkspaceConnectionInfo::Ssh { auth_method, .. }
                if auth_method == "keyboard_interactive"
        ));
    }

    #[test]
    fn automatic_ssh_history_is_valid_without_a_connection_key() {
        let mut input = ssh_input("router.example", "operator");
        if let WorkspaceConnectionInfo::Ssh {
            auth_method,
            private_key_path,
            ..
        } = &mut input.connection_info
        {
            *auth_method = "auto".into();
            *private_key_path = None;
        }

        let normalized = normalize_input(input).unwrap();
        assert!(matches!(
            normalized.connection_info,
            WorkspaceConnectionInfo::Ssh {
                auth_method,
                private_key_path: None,
                ..
            } if auth_method == "auto"
        ));
    }

    fn telnet_input(host: &str, port: u16) -> ConnectionHistoryRecordInput {
        ConnectionHistoryRecordInput {
            connection_info: WorkspaceConnectionInfo::Telnet {
                host: host.into(),
                port,
            },
            encoding: "shift-jis".into(),
            terminal_mode: "cisco_ios".into(),
        }
    }

    fn at(second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, second)
            .single()
            .unwrap()
    }

    #[test]
    fn missing_file_loads_as_empty_history() {
        let path = temp_path();
        assert_eq!(
            read_history(&path).unwrap(),
            ConnectionHistoryFile::default()
        );
    }

    #[test]
    fn round_trip_preserves_non_secret_connection_fields() {
        let path = temp_path();
        let mut history = ConnectionHistoryFile::default();
        upsert_history(&mut history, ssh_input("router.example", "admin"), at(1)).unwrap();
        write_history(&path, &history).unwrap();

        let loaded = read_history(&path).unwrap();
        assert_eq!(loaded, history);
        let serialized: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let connection_info = &serialized["entries"][0]["connection_info"];
        assert!(connection_info.get("password").is_none());
        assert!(connection_info.get("key_passphrase").is_none());
        assert!(connection_info.get("jump_password").is_none());
        assert!(connection_info.get("jump_key_passphrase").is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn ssh_upsert_normalizes_host_and_preserves_id() {
        let mut history = ConnectionHistoryFile::default();
        upsert_history(&mut history, ssh_input(" Router.Example ", "admin"), at(1)).unwrap();
        let id = history.entries[0].id.clone();
        let mut updated = ssh_input("router.example", "admin");
        updated.encoding = "euc-jp".into();
        upsert_history(&mut history, updated, at(2)).unwrap();

        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].id, id);
        assert_eq!(history.entries[0].encoding, "euc-jp");
        assert_eq!(history.entries[0].last_connected_at, at(2));
    }

    #[test]
    fn accepts_device_terminal_modes_and_rejects_unknown_modes() {
        let mut history = ConnectionHistoryFile::default();
        let mut eos_input = ssh_input("switch.example", "admin");
        eos_input.terminal_mode = "arista_eos".into();
        upsert_history(&mut history, eos_input, at(1)).unwrap();

        assert_eq!(history.entries[0].terminal_mode, "arista_eos");

        let mut junos_input = ssh_input("junos.example", "admin");
        junos_input.terminal_mode = "juniper_junos".into();
        upsert_history(&mut history, junos_input, at(2)).unwrap();

        assert_eq!(history.entries[0].terminal_mode, "juniper_junos");

        let mut vyos_input = ssh_input("vyos.example", "vyos");
        vyos_input.terminal_mode = "vyos".into();
        upsert_history(&mut history, vyos_input, at(3)).unwrap();

        assert_eq!(history.entries[0].terminal_mode, "vyos");

        let mut sir_input = ssh_input("sir.example", "admin");
        sir_input.terminal_mode = "fujitsu_sir".into();
        upsert_history(&mut history, sir_input, at(4)).unwrap();

        assert_eq!(history.entries[0].terminal_mode, "fujitsu_sir");

        let mut awplus_input = ssh_input("awplus.example", "manager");
        awplus_input.terminal_mode = "allied_telesis_awplus".into();
        upsert_history(&mut history, awplus_input, at(5)).unwrap();

        assert_eq!(history.entries[0].terminal_mode, "allied_telesis_awplus");

        let mut fitelnet_input = ssh_input("fitelnet.example", "operator");
        fitelnet_input.terminal_mode = "furukawa_fitelnet".into();
        upsert_history(&mut history, fitelnet_input, at(6)).unwrap();

        assert_eq!(history.entries[0].terminal_mode, "furukawa_fitelnet");

        let mut unknown_input = ssh_input("router.example", "admin");
        unknown_input.terminal_mode = "unknown".into();
        assert!(upsert_history(&mut history, unknown_input, at(7)).is_err());
    }

    #[test]
    fn ssh_usernames_remain_case_sensitive() {
        let mut history = ConnectionHistoryFile::default();
        upsert_history(&mut history, ssh_input("router.example", "admin"), at(1)).unwrap();
        upsert_history(&mut history, ssh_input("ROUTER.EXAMPLE", "Admin"), at(2)).unwrap();
        assert_eq!(history.entries.len(), 2);
    }

    #[test]
    fn limits_ssh_and_telnet_independently() {
        let mut history = ConnectionHistoryFile::default();
        for index in 0..12 {
            upsert_history(
                &mut history,
                ssh_input(&format!("ssh-{index}.example"), "admin"),
                at(index),
            )
            .unwrap();
            upsert_history(
                &mut history,
                telnet_input(&format!("telnet-{index}.example"), 23),
                at(index),
            )
            .unwrap();
        }

        assert_eq!(
            history.entries.iter().filter(|entry| is_ssh(entry)).count(),
            10
        );
        assert_eq!(
            history
                .entries
                .iter()
                .filter(|entry| !is_ssh(entry))
                .count(),
            10
        );
        assert!(history
            .entries
            .windows(2)
            .all(|entries| entries[0].last_connected_at >= entries[1].last_connected_at));
    }

    #[test]
    fn malformed_history_is_not_overwritten_by_recording() {
        let path = temp_path();
        fs::write(&path, "not-json").unwrap();
        let before = fs::read_to_string(&path).unwrap();
        let result = read_history(&path).and_then(|mut history| {
            upsert_history(&mut history, ssh_input("router.example", "admin"), at(1))?;
            write_history(&path, &history)
        });

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn individual_delete_is_idempotent() {
        let mut history = ConnectionHistoryFile::default();
        upsert_history(&mut history, ssh_input("router.example", "admin"), at(1)).unwrap();
        let id = history.entries[0].id.clone();

        assert!(remove_history_entry(&mut history, &id));
        assert!(!remove_history_entry(&mut history, &id));
        assert!(history.entries.is_empty());
    }

    #[test]
    fn clear_removes_malformed_file_and_missing_file_is_allowed() {
        let path = temp_path();
        fs::write(&path, "not-json").unwrap();
        clear_history_file(&path).unwrap();
        assert!(!path.exists());
        clear_history_file(&path).unwrap();
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let path = temp_path();
        fs::write(&path, r#"{"version":2,"entries":[]}"#).unwrap();
        assert!(read_history(&path)
            .unwrap_err()
            .contains("Unsupported connection history version"));
        fs::remove_file(path).unwrap();
    }
}
