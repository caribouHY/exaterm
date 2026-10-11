use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSession {
    pub session_id: String,
    pub connection_type: String, // "ssh", "serial", or "telnet"
    pub target: String,          // e.g. "user@host:22", "COM3", or "host:23"
    pub started_at: String,
    pub file_path: String,
    #[serde(default = "default_log_mode")]
    pub log_mode: String,
}

#[derive(Clone)]
pub struct LoggerState {
    log_dir: PathBuf,
    index_path: PathBuf,
    sessions: Arc<Mutex<HashMap<String, LogSession>>>,
}

impl LoggerState {
    pub fn new() -> Self {
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ExaTerm")
            .join("logs");
        let _ = fs::create_dir_all(&log_dir);
        let index_path = log_dir.join("index.json");
        Self {
            log_dir,
            index_path,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub fn with_paths(log_dir: PathBuf, index_path: PathBuf) -> Self {
        let _ = fs::create_dir_all(&log_dir);
        Self {
            log_dir,
            index_path,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn default_log_mode() -> String {
    "auto".into()
}

fn read_log_index(index_path: &PathBuf) -> Result<Vec<LogSession>, String> {
    if !index_path.exists() {
        return Ok(Vec::new());
    }

    let data =
        fs::read_to_string(index_path).map_err(|e| format!("Failed to read log history: {}", e))?;
    let mut sessions: Vec<LogSession> =
        serde_json::from_str(&data).map_err(|e| format!("Failed to parse log history: {}", e))?;
    sort_sessions_desc(&mut sessions);
    Ok(sessions)
}

fn write_log_index(index_path: &PathBuf, sessions: &[LogSession]) -> Result<(), String> {
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create the log history directory: {}", e))?;
    }

    let data = serde_json::to_string_pretty(sessions)
        .map_err(|e| format!("Failed to serialize log history: {}", e))?;
    fs::write(index_path, data).map_err(|e| format!("Failed to save log history: {}", e))
}

fn upsert_log_session(index_path: &PathBuf, session: LogSession) -> Result<(), String> {
    let mut sessions = read_log_index(index_path)?;
    if session.log_mode == "manual" {
        sessions.push(session);
    } else {
        upsert_non_manual_log_session(&mut sessions, session);
    }
    sort_sessions_desc(&mut sessions);
    write_log_index(index_path, &sessions)
}

fn upsert_non_manual_log_session(sessions: &mut Vec<LogSession>, session: LogSession) {
    if let Some(existing) = sessions
        .iter_mut()
        .find(|item| item.session_id == session.session_id && item.log_mode == session.log_mode)
    {
        *existing = session;
    } else {
        sessions.push(session);
    }
}

fn sort_sessions_desc(sessions: &mut [LogSession]) {
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LogBulkDeleteResult {
    pub removed_history_count: usize,
    pub removed_auto_file_count: usize,
    pub skipped_active_count: usize,
    pub skipped_manual_file_count: usize,
    pub skipped_missing_file_count: usize,
    pub skipped_unsafe_path_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ActiveLogKey {
    session_id: String,
    log_mode: String,
    file_path: String,
}

enum AutoLogFileDeleteResult {
    Removed,
    Missing,
    UnsafePath,
}

fn is_session_active(session: &LogSession, active_keys: &HashSet<ActiveLogKey>) -> bool {
    active_keys.contains(&ActiveLogKey {
        session_id: session.session_id.clone(),
        log_mode: session.log_mode.clone(),
        file_path: session.file_path.clone(),
    })
}

fn active_log_keys(sessions: &HashMap<String, LogSession>) -> HashSet<ActiveLogKey> {
    sessions
        .iter()
        .map(|(session_id, session)| ActiveLogKey {
            session_id: session_id.clone(),
            log_mode: session.log_mode.clone(),
            file_path: session.file_path.clone(),
        })
        .collect()
}

fn protected_log_history_rows(
    sessions: &[LogSession],
    active_keys: &HashSet<ActiveLogKey>,
    delete_auto_files: bool,
) -> Result<Vec<bool>, String> {
    // Keep handles open during comparison so file identities stay valid and aliases,
    // including hard links, cannot bypass protection through different path strings.
    let active_files = if delete_auto_files {
        active_keys
            .iter()
            .map(|key| {
                fs::canonicalize(&key.file_path)
                    .and_then(same_file::Handle::from_path)
                    .map_err(|e| format!("Failed to verify an active log file: {}", e))
            })
            .collect::<Result<HashSet<_>, _>>()?
    } else {
        HashSet::new()
    };

    sessions
        .iter()
        .map(|session| {
            if is_session_active(session, active_keys) {
                return Ok(true);
            }
            if active_files.is_empty()
                || !matches!(session.log_mode.as_str(), "auto" | "manual")
                || !Path::new(&session.file_path).exists()
            {
                return Ok(false);
            }
            let file = fs::canonicalize(&session.file_path)
                .and_then(same_file::Handle::from_path)
                .map_err(|e| format!("Failed to verify the log file: {}", e))?;
            Ok(active_files.contains(&file))
        })
        .collect()
}

fn delete_auto_log_file(
    log_dir: &PathBuf,
    file_path: &str,
) -> Result<AutoLogFileDeleteResult, String> {
    let log_dir = fs::canonicalize(log_dir)
        .map_err(|e| format!("Failed to verify the log directory: {}", e))?;
    let file_path = PathBuf::from(file_path);

    if file_path.exists() {
        let canonical_file = fs::canonicalize(&file_path)
            .map_err(|e| format!("Failed to verify the log file: {}", e))?;
        if !canonical_file.starts_with(&log_dir) {
            return Ok(AutoLogFileDeleteResult::UnsafePath);
        }
        fs::remove_file(&canonical_file)
            .map_err(|e| format!("Failed to delete the log file: {}", e))?;
        return Ok(AutoLogFileDeleteResult::Removed);
    }

    if let Some(parent) = file_path.parent() {
        if let Ok(canonical_parent) = fs::canonicalize(parent) {
            if canonical_parent.starts_with(&log_dir) {
                return Ok(AutoLogFileDeleteResult::Missing);
            }
        }
    }

    Ok(AutoLogFileDeleteResult::UnsafePath)
}

fn bulk_delete_log_sessions(
    index_path: &PathBuf,
    log_dir: &PathBuf,
    active_keys: &HashSet<ActiveLogKey>,
    delete_auto_files: bool,
) -> Result<LogBulkDeleteResult, String> {
    let sessions = read_log_index(index_path)?;
    // Complete protection checks before deleting anything; unresolved active paths
    // must not let an older history entry delete a file that is still being recorded.
    let protected = protected_log_history_rows(&sessions, active_keys, delete_auto_files)?;
    let mut kept = Vec::new();
    let mut result = LogBulkDeleteResult::default();

    for (session, is_protected) in sessions.into_iter().zip(protected) {
        if is_protected {
            result.skipped_active_count += 1;
            kept.push(session);
            continue;
        }

        match session.log_mode.as_str() {
            "auto" => {
                result.removed_history_count += 1;
                if delete_auto_files {
                    match delete_auto_log_file(log_dir, &session.file_path)? {
                        AutoLogFileDeleteResult::Removed => result.removed_auto_file_count += 1,
                        AutoLogFileDeleteResult::Missing => result.skipped_missing_file_count += 1,
                        AutoLogFileDeleteResult::UnsafePath => {
                            result.skipped_unsafe_path_count += 1
                        }
                    }
                }
            }
            "manual" => {
                result.removed_history_count += 1;
                if delete_auto_files {
                    result.skipped_manual_file_count += 1;
                }
            }
            _ => kept.push(session),
        }
    }

    sort_sessions_desc(&mut kept);
    write_log_index(index_path, &kept)?;
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogWriteMode {
    Overwrite,
    Append,
}

struct LogSessionCreateOptions {
    session_id: String,
    connection_type: String,
    target: String,
    file_path: Option<String>,
    log_mode: String,
    include_header: bool,
    write_mode: LogWriteMode,
}

struct LogStartOptions {
    session_id: String,
    connection_type: String,
    target: String,
    file_path: Option<String>,
    log_mode: String,
    write_mode: LogWriteMode,
}

impl LogWriteMode {
    fn from_optional_str(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("overwrite") {
            "overwrite" => Ok(Self::Overwrite),
            "append" => Ok(Self::Append),
            other => Err(format!("Unknown log write mode: {}", other)),
        }
    }
}

fn write_log_start_header(
    file_path: &PathBuf,
    connection_type: &str,
    target: &str,
    log_mode: &str,
    include_header: bool,
    write_mode: LogWriteMode,
    started_at: &str,
) -> Result<(), String> {
    match write_mode {
        LogWriteMode::Overwrite => {
            if include_header {
                let header = format!(
                    "# ExaTerm Log\n# Type: {}\n# Target: {}\n# Mode: {}\n# Started: {}\n\n",
                    connection_type, target, log_mode, started_at
                );
                fs::write(file_path, &header)
                    .map_err(|e| format!("Failed to create the log file: {}", e))?;
            } else {
                fs::write(file_path, "")
                    .map_err(|e| format!("Failed to create the log file: {}", e))?;
            }
        }
        LogWriteMode::Append => {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .read(true)
                .append(true)
                .open(file_path)
                .map_err(|e| format!("Failed to create the log file: {}", e))?;

            if include_header {
                if file
                    .metadata()
                    .map_err(|e| format!("Failed to create the log file: {}", e))?
                    .len()
                    > 0
                {
                    file.seek(SeekFrom::End(-1))
                        .map_err(|e| format!("Failed to create the log file: {}", e))?;
                    let mut last_byte = [0_u8; 1];
                    file.read_exact(&mut last_byte)
                        .map_err(|e| format!("Failed to create the log file: {}", e))?;
                    if last_byte[0] != b'\n' {
                        writeln!(file)
                            .map_err(|e| format!("Failed to create the log file: {}", e))?;
                    }
                }

                let header = format!("# ExaTerm Log Append\n# Started: {}\n\n", started_at);
                write!(file, "{}", header)
                    .map_err(|e| format!("Failed to create the log file: {}", e))?;
            }
        }
    }

    Ok(())
}

fn create_log_session(
    log_dir: &Path,
    options: LogSessionCreateOptions,
) -> Result<LogSession, String> {
    let LogSessionCreateOptions {
        session_id,
        connection_type,
        target,
        file_path,
        log_mode,
        include_header,
        write_mode,
    } = options;
    let now = Local::now();
    let started_at = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let session_prefix = session_id.chars().take(8).collect::<String>();
    let filename = format!(
        "{}_{:09}_{}.log",
        now.format("%Y%m%d_%H%M%S"),
        now.timestamp_subsec_nanos(),
        session_prefix
    );
    let file_path = file_path
        .map(PathBuf::from)
        .unwrap_or_else(|| log_dir.join(&filename));
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create the log directory: {}", e))?;
    }
    write_log_start_header(
        &file_path,
        &connection_type,
        &target,
        &log_mode,
        include_header,
        write_mode,
        &started_at,
    )?;

    Ok(LogSession {
        session_id: session_id.clone(),
        connection_type,
        target,
        started_at: now.to_rfc3339(),
        file_path: file_path.to_string_lossy().to_string(),
        log_mode,
    })
}

fn append_to_log_sessions(sessions: &[LogSession], data: &str) -> Result<(), String> {
    for session in sessions {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&session.file_path)
            .map_err(|e| format!("Failed to write to the log file: {}", e))?;
        use std::io::Write;
        write!(file, "{}", data).map_err(|e| format!("Failed to write to the log file: {}", e))?;
    }
    Ok(())
}

fn command_result<T>(
    result: Result<T, String>,
) -> Result<T, crate::command_error::BackendCommandError> {
    result.map_err(Into::into)
}

pub async fn start_log_on_connection(
    state: &LoggerState,
    session_id: String,
    connection_type: String,
    target: String,
) -> Result<String, String> {
    start_log(
        state,
        LogStartOptions {
            session_id,
            connection_type,
            target,
            file_path: None,
            log_mode: "auto".into(),
            write_mode: LogWriteMode::Overwrite,
        },
        false,
    )
    .await
}

#[tauri::command]
pub async fn logger_start_on_connection(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    connection_type: String,
    target: String,
) -> Result<String, crate::command_error::BackendCommandError> {
    command_result(start_log_on_connection(&state, session_id, connection_type, target).await)
}

#[tauri::command]
pub async fn logger_start_manual(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    connection_type: String,
    target: String,
    file_path: Option<String>,
    write_mode: Option<String>,
) -> Result<String, crate::command_error::BackendCommandError> {
    command_result(
        start_manual_log(
            &state,
            session_id,
            connection_type,
            target,
            file_path,
            write_mode,
        )
        .await,
    )
}

pub async fn start_manual_log(
    state: &LoggerState,
    session_id: String,
    connection_type: String,
    target: String,
    file_path: Option<String>,
    write_mode: Option<String>,
) -> Result<String, String> {
    let write_mode = LogWriteMode::from_optional_str(write_mode.as_deref())?;
    start_log(
        state,
        LogStartOptions {
            session_id,
            connection_type,
            target,
            file_path,
            log_mode: "manual".into(),
            write_mode,
        },
        true,
    )
    .await
}

async fn start_log(
    state: &LoggerState,
    options: LogStartOptions,
    reuse_existing: bool,
) -> Result<String, String> {
    let mut sessions = state.sessions.lock().await;
    if reuse_existing {
        if let Some(session) = sessions.get(&options.session_id) {
            if session.log_mode == "manual" {
                return Ok(session.file_path.clone());
            }
        }
    }
    let include_header = crate::config::config_read()
        .map(|cfg| cfg.terminal.include_log_header)
        .unwrap_or(true);
    let session_id = options.session_id.clone();
    let session = create_log_session(
        &state.log_dir,
        LogSessionCreateOptions {
            session_id: options.session_id,
            connection_type: options.connection_type,
            target: options.target,
            file_path: options.file_path,
            log_mode: options.log_mode,
            include_header,
            write_mode: options.write_mode,
        },
    )?;
    upsert_log_session(&state.index_path, session.clone())?;
    sessions.insert(session_id, session.clone());
    Ok(session.file_path)
}

#[tauri::command]
pub async fn logger_stop_manual(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    command_result(stop_manual_log(&state, &session_id).await)
}

pub async fn stop_manual_log(state: &LoggerState, session_id: &str) -> Result<(), String> {
    state.sessions.lock().await.remove(session_id);
    Ok(())
}

pub async fn clear_session_logs(state: &LoggerState, session_id: &str) {
    state.sessions.lock().await.remove(session_id);
}

pub async fn active_log_session(state: &LoggerState, session_id: &str) -> Option<LogSession> {
    state.sessions.lock().await.get(session_id).cloned()
}

#[cfg(test)]
pub async fn manual_log_session(state: &LoggerState, session_id: &str) -> Option<LogSession> {
    active_log_session(state, session_id).await
}

#[tauri::command]
pub async fn logger_is_manual_active(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<bool, String> {
    Ok(active_log_session(&state, &session_id).await.is_some())
}

#[tauri::command]
pub async fn logger_append(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    data: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    let active_sessions = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(&session_id)
            .cloned()
            .into_iter()
            .collect::<Vec<_>>()
    };
    command_result(append_to_log_sessions(&active_sessions, &data))
}

#[tauri::command]
pub async fn logger_get_sessions(
    state: tauri::State<'_, LoggerState>,
) -> Result<Vec<LogSession>, crate::command_error::BackendCommandError> {
    let _sessions = state.sessions.lock().await;
    command_result(read_log_index(&state.index_path))
}

#[tauri::command]
pub async fn logger_bulk_delete_sessions(
    state: tauri::State<'_, LoggerState>,
    delete_auto_files: bool,
) -> Result<LogBulkDeleteResult, crate::command_error::BackendCommandError> {
    command_result(delete_log_sessions(&state, delete_auto_files).await)
}

async fn delete_log_sessions(
    state: &LoggerState,
    delete_auto_files: bool,
) -> Result<LogBulkDeleteResult, String> {
    // Starting a log uses this same lock, so its file cannot become active between
    // the protection check and deletion or be lost from a concurrent index update.
    let sessions = state.sessions.lock().await;
    let active_keys = active_log_keys(&sessions);
    bulk_delete_log_sessions(
        &state.index_path,
        &state.log_dir,
        &active_keys,
        delete_auto_files,
    )
}

#[tauri::command]
pub fn logger_get_log_dir(state: tauri::State<'_, LoggerState>) -> String {
    state.log_dir.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests;
