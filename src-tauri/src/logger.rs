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

#[cfg(windows)]
fn normalized_paths_equal(left: &Path, right: &Path) -> Result<bool, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

    let left: Vec<u16> = left.as_os_str().encode_wide().collect();
    let right: Vec<u16> = right.as_os_str().encode_wide().collect();
    let left_len = i32::try_from(left.len()).map_err(|_| "The log path is too long")?;
    let right_len = i32::try_from(right.len()).map_err(|_| "The log path is too long")?;
    // Both UTF-16 buffers remain alive for the explicit lengths passed to Windows.
    let result =
        unsafe { CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) };
    if result == 0 {
        return Err(format!(
            "Failed to compare log paths: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(result == CSTR_EQUAL)
}

#[cfg(not(windows))]
fn normalized_paths_equal(left: &Path, right: &Path) -> Result<bool, String> {
    Ok(left == right)
}

fn protected_log_history_rows(
    sessions: &[LogSession],
    active_keys: &HashSet<ActiveLogKey>,
    delete_auto_files: bool,
) -> Result<Vec<bool>, String> {
    let active_paths = if delete_auto_files {
        active_keys
            .iter()
            .map(|key| {
                fs::canonicalize(&key.file_path)
                    .map_err(|e| format!("Failed to verify an active log file: {}", e))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };

    sessions
        .iter()
        .map(|session| {
            if is_session_active(session, active_keys) {
                return Ok(true);
            }
            if active_paths.is_empty()
                || !matches!(session.log_mode.as_str(), "auto" | "manual")
                || !Path::new(&session.file_path).exists()
            {
                return Ok(false);
            }
            let path = fs::canonicalize(&session.file_path)
                .map_err(|e| format!("Failed to verify the log file: {}", e))?;
            for active_path in &active_paths {
                if normalized_paths_equal(&path, active_path)? {
                    return Ok(true);
                }
            }
            Ok(false)
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
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_index_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("exaterm_logger_test_{}", Uuid::new_v4()))
            .join("index.json")
    }

    fn sample_session(session_id: &str, started_at: &str, target: &str) -> LogSession {
        LogSession {
            session_id: session_id.into(),
            connection_type: "ssh".into(),
            target: target.into(),
            started_at: started_at.into(),
            file_path: format!("C:\\logs\\{}.log", session_id),
            log_mode: "auto".into(),
        }
    }

    fn cleanup(path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    fn empty_active_keys() -> HashSet<ActiveLogKey> {
        HashSet::new()
    }

    #[test]
    fn read_log_index_returns_empty_when_missing() {
        let path = temp_index_path();
        let sessions = read_log_index(&path).expect("missing index should read as empty");

        assert!(sessions.is_empty());
        cleanup(&path);
    }

    #[test]
    fn write_and_read_log_index_round_trips_sessions() {
        let path = temp_index_path();
        let sessions = vec![sample_session(
            "session-1",
            "2026-04-25T10:00:00+09:00",
            "user@host:22",
        )];

        write_log_index(&path, &sessions).expect("index should write");
        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].session_id, "session-1");
        assert_eq!(loaded[0].target, "user@host:22");
        assert_eq!(loaded[0].log_mode, "auto");
        cleanup(&path);
    }

    #[test]
    fn upsert_log_session_replaces_existing_session_id() {
        let path = temp_index_path();
        upsert_log_session(
            &path,
            sample_session("session-1", "2026-04-25T10:00:00+09:00", "old@host:22"),
        )
        .expect("initial upsert should write");
        upsert_log_session(
            &path,
            sample_session("session-1", "2026-04-25T11:00:00+09:00", "new@host:22"),
        )
        .expect("second upsert should replace");

        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].target, "new@host:22");
        assert_eq!(loaded[0].started_at, "2026-04-25T11:00:00+09:00");
        cleanup(&path);
    }

    #[test]
    fn upsert_log_session_keeps_auto_and_manual_entries() {
        let path = temp_index_path();
        let auto = sample_session("session-1", "2026-04-25T10:00:00+09:00", "host");
        let mut manual = sample_session("session-1", "2026-04-25T10:01:00+09:00", "host");
        manual.log_mode = "manual".into();
        manual.file_path = "C:\\manual\\session-1.log".into();

        upsert_log_session(&path, auto).expect("auto upsert should write");
        upsert_log_session(&path, manual).expect("manual upsert should write");
        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(loaded.len(), 2);
        assert!(loaded.iter().any(|entry| entry.log_mode == "auto"));
        assert!(loaded.iter().any(|entry| entry.log_mode == "manual"));
        cleanup(&path);
    }

    #[test]
    fn upsert_log_session_keeps_multiple_manual_entries_for_same_session() {
        let path = temp_index_path();
        let mut first = sample_session("session-1", "2026-04-25T10:00:00+09:00", "host");
        first.log_mode = "manual".into();
        first.file_path = "C:\\manual\\first.log".into();
        let mut second = sample_session("session-1", "2026-04-25T11:00:00+09:00", "host");
        second.log_mode = "manual".into();
        second.file_path = "C:\\manual\\second.log".into();

        upsert_log_session(&path, first).expect("first manual upsert should write");
        upsert_log_session(&path, second).expect("second manual upsert should append");
        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].file_path, "C:\\manual\\second.log");
        assert_eq!(loaded[1].file_path, "C:\\manual\\first.log");
        cleanup(&path);
    }

    #[test]
    fn append_to_log_sessions_writes_to_active_target() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let log_path = dir.join("session.log");
        fs::write(&log_path, "log\n").expect("log file should be created");
        let sessions = vec![LogSession {
            file_path: log_path.to_string_lossy().to_string(),
            ..sample_session("session-1", "2026-04-25T10:00:00+09:00", "host")
        }];

        append_to_log_sessions(&sessions, "data\n").expect("append should write the log");

        assert_eq!(fs::read_to_string(&log_path).unwrap(), "log\ndata\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_log_session_writes_header_when_enabled() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");

        let session = create_log_session(
            &dir,
            LogSessionCreateOptions {
                session_id: "session-1".into(),
                connection_type: "ssh".into(),
                target: "user@host:22".into(),
                file_path: None,
                log_mode: "auto".into(),
                include_header: true,
                write_mode: LogWriteMode::Overwrite,
            },
        )
        .expect("log session should be created");
        let data = fs::read_to_string(&session.file_path).expect("log should read");

        assert!(data.starts_with("# ExaTerm Log\n"));
        assert!(data.contains("# Type: ssh\n"));
        assert!(data.contains("# Target: user@host:22\n"));
        assert!(data.contains("# Mode: auto\n"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_log_session_skips_header_when_disabled() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");

        let session = create_log_session(
            &dir,
            LogSessionCreateOptions {
                session_id: "session-1".into(),
                connection_type: "ssh".into(),
                target: "user@host:22".into(),
                file_path: None,
                log_mode: "auto".into(),
                include_header: false,
                write_mode: LogWriteMode::Overwrite,
            },
        )
        .expect("log session should be created");
        let data = fs::read_to_string(&session.file_path).expect("log should read");

        assert_eq!(data, "");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn start_manual_log_without_file_path_uses_log_dir() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        let index_path = dir.join("index.json");
        let state = LoggerState::with_paths(dir.clone(), index_path.clone());

        let file_path = start_manual_log(
            &state,
            "session-1".into(),
            "ssh".into(),
            "user@host:22".into(),
            None,
            None,
        )
        .await
        .expect("manual log should start");
        let path = PathBuf::from(&file_path);
        let parent = path.parent().expect("manual log should have parent");

        assert_eq!(parent, dir.as_path());
        assert!(path.exists());

        let loaded = read_log_index(&index_path).expect("index should read");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].log_mode, "manual");
        assert_eq!(loaded[0].file_path, file_path);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn repeated_manual_start_reuses_the_active_file_without_overwriting_it() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let index_path = dir.join("index.json");
        let first_path = dir.join("first.log");
        let second_path = dir.join("second.log");
        let state = LoggerState::with_paths(dir.clone(), index_path);

        let active_path = start_manual_log(
            &state,
            "session-1".into(),
            "ssh".into(),
            "host".into(),
            Some(first_path.to_string_lossy().to_string()),
            Some("overwrite".into()),
        )
        .await
        .expect("first manual log should start");
        fs::write(&first_path, "preserved content\n").expect("active log should be writable");

        let repeated_path = start_manual_log(
            &state,
            "session-1".into(),
            "ssh".into(),
            "host".into(),
            Some(second_path.to_string_lossy().to_string()),
            Some("overwrite".into()),
        )
        .await
        .expect("repeated manual log start should be idempotent");

        assert_eq!(repeated_path, active_path);
        assert_eq!(
            fs::read_to_string(first_path).unwrap(),
            "preserved content\n"
        );
        assert!(!second_path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn connection_and_manual_starts_share_one_active_target() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        let index_path = dir.join("index.json");
        let state = LoggerState::with_paths(dir.clone(), index_path.clone());

        start_log_on_connection(
            &state,
            "session-1".into(),
            "ssh".into(),
            "user@host:22".into(),
        )
        .await
        .expect("connection log should start");
        let auto_session = manual_log_session(&state, "session-1")
            .await
            .expect("connection log should be active");
        assert_eq!(auto_session.log_mode, "auto");
        start_manual_log(
            &state,
            "session-1".into(),
            "ssh".into(),
            "user@host:22".into(),
            None,
            None,
        )
        .await
        .expect("manual log should start");
        let manual_session = manual_log_session(&state, "session-1")
            .await
            .expect("manual log should replace the active target");
        assert_eq!(manual_session.log_mode, "manual");
        assert_ne!(manual_session.file_path, auto_session.file_path);
        assert_eq!(state.sessions.lock().await.len(), 1);

        clear_session_logs(&state, "session-1").await;
        let active_keys = {
            let sessions = state.sessions.lock().await;
            active_log_keys(&sessions)
        };
        let result = bulk_delete_log_sessions(&index_path, &dir, &active_keys, false)
            .expect("bulk delete should succeed");
        let loaded = read_log_index(&index_path).expect("index should read");

        assert_eq!(result.skipped_active_count, 0);
        assert_eq!(result.removed_history_count, 2);
        assert!(loaded.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_log_session_overwrite_replaces_existing_file() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let path = dir.join("manual.log");
        fs::write(&path, "existing content\n").expect("manual file should be created");

        let session = create_log_session(
            &dir,
            LogSessionCreateOptions {
                session_id: "session-1".into(),
                connection_type: "ssh".into(),
                target: "user@host:22".into(),
                file_path: Some(path.to_string_lossy().to_string()),
                log_mode: "manual".into(),
                include_header: true,
                write_mode: LogWriteMode::Overwrite,
            },
        )
        .expect("log session should be created");
        let data = fs::read_to_string(&session.file_path).expect("log should read");

        assert!(data.starts_with("# ExaTerm Log\n"));
        assert!(data.contains("# Mode: manual\n"));
        assert!(!data.contains("existing content"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_log_session_appends_header_to_existing_file() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let path = dir.join("manual.log");
        fs::write(&path, "existing content").expect("manual file should be created");

        let session = create_log_session(
            &dir,
            LogSessionCreateOptions {
                session_id: "session-1".into(),
                connection_type: "ssh".into(),
                target: "user@host:22".into(),
                file_path: Some(path.to_string_lossy().to_string()),
                log_mode: "manual".into(),
                include_header: true,
                write_mode: LogWriteMode::Append,
            },
        )
        .expect("log session should be created");
        let data = fs::read_to_string(&session.file_path).expect("log should read");

        assert!(data.starts_with("existing content\n# ExaTerm Log Append\n"));
        assert!(data.contains("# Started: "));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_log_session_append_skips_header_when_disabled() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let path = dir.join("manual.log");
        fs::write(&path, "existing content\n").expect("manual file should be created");

        let session = create_log_session(
            &dir,
            LogSessionCreateOptions {
                session_id: "session-1".into(),
                connection_type: "ssh".into(),
                target: "user@host:22".into(),
                file_path: Some(path.to_string_lossy().to_string()),
                log_mode: "manual".into(),
                include_header: false,
                write_mode: LogWriteMode::Append,
            },
        )
        .expect("log session should be created");
        append_to_log_sessions(&[session.clone()], "new content\n")
            .expect("manual append should write");
        let data = fs::read_to_string(&session.file_path).expect("log should read");

        assert_eq!(data, "existing content\nnew content\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_log_index_sorts_sessions_by_started_at_desc() {
        let path = temp_index_path();
        let sessions = vec![
            sample_session("older", "2026-04-25T09:00:00+09:00", "older@host:22"),
            sample_session("newer", "2026-04-25T11:00:00+09:00", "newer@host:22"),
            sample_session("middle", "2026-04-25T10:00:00+09:00", "middle@host:22"),
        ];

        write_log_index(&path, &sessions).expect("index should write");
        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(loaded[0].session_id, "newer");
        assert_eq!(loaded[1].session_id, "middle");
        assert_eq!(loaded[2].session_id, "older");
        cleanup(&path);
    }

    #[test]
    fn bulk_delete_removes_inactive_auto_and_manual_history() {
        let path = temp_index_path();
        let log_dir = path.parent().unwrap().to_path_buf();
        let auto = sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host");
        let mut manual = sample_session("manual-1", "2026-04-25T11:00:00+09:00", "host");
        manual.log_mode = "manual".into();

        write_log_index(&path, &[auto, manual]).expect("index should write");
        let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), false)
            .expect("bulk delete should succeed");
        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(result.removed_history_count, 2);
        assert!(loaded.is_empty());
        cleanup(&path);
    }

    #[test]
    fn bulk_delete_history_only_leaves_files() {
        let path = temp_index_path();
        let log_dir = path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&log_dir).expect("log dir should be created");
        let auto_path = log_dir.join("auto.log");
        let manual_path = log_dir.join("manual.log");
        fs::write(&auto_path, "auto").expect("auto file should be created");
        fs::write(&manual_path, "manual").expect("manual file should be created");
        let auto = LogSession {
            file_path: auto_path.to_string_lossy().to_string(),
            ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
        };
        let manual = LogSession {
            file_path: manual_path.to_string_lossy().to_string(),
            log_mode: "manual".into(),
            ..sample_session("manual-1", "2026-04-25T11:00:00+09:00", "host")
        };

        write_log_index(&path, &[auto, manual]).expect("index should write");
        let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), false)
            .expect("bulk delete should succeed");

        assert_eq!(result.removed_history_count, 2);
        assert!(auto_path.exists());
        assert!(manual_path.exists());
        cleanup(&path);
    }

    #[test]
    fn bulk_delete_with_files_removes_only_auto_files() {
        let path = temp_index_path();
        let log_dir = path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&log_dir).expect("log dir should be created");
        let auto_path = log_dir.join("auto.log");
        let manual_path = log_dir.join("manual.log");
        fs::write(&auto_path, "auto").expect("auto file should be created");
        fs::write(&manual_path, "manual").expect("manual file should be created");
        let auto = LogSession {
            file_path: auto_path.to_string_lossy().to_string(),
            ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
        };
        let manual = LogSession {
            file_path: manual_path.to_string_lossy().to_string(),
            log_mode: "manual".into(),
            ..sample_session("manual-1", "2026-04-25T11:00:00+09:00", "host")
        };

        write_log_index(&path, &[auto, manual]).expect("index should write");
        let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), true)
            .expect("bulk delete should succeed");

        assert_eq!(result.removed_history_count, 2);
        assert_eq!(result.removed_auto_file_count, 1);
        assert_eq!(result.skipped_manual_file_count, 1);
        assert!(!auto_path.exists());
        assert!(manual_path.exists());
        cleanup(&path);
    }

    #[test]
    fn bulk_delete_keeps_active_logs() {
        let path = temp_index_path();
        let log_dir = path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&log_dir).expect("log dir should be created");
        let auto_path = log_dir.join("auto.log");
        let manual_path = log_dir.join("manual.log");
        fs::write(&auto_path, "auto").expect("auto file should be created");
        fs::write(&manual_path, "manual").expect("manual file should be created");
        let auto = LogSession {
            file_path: auto_path.to_string_lossy().to_string(),
            ..sample_session("active", "2026-04-25T10:00:00+09:00", "host")
        };
        let manual = LogSession {
            file_path: manual_path.to_string_lossy().to_string(),
            log_mode: "manual".into(),
            ..sample_session("active", "2026-04-25T11:00:00+09:00", "host")
        };
        let active = HashSet::from([
            ActiveLogKey {
                session_id: "active".into(),
                log_mode: "auto".into(),
                file_path: auto_path.to_string_lossy().to_string(),
            },
            ActiveLogKey {
                session_id: "active".into(),
                log_mode: "manual".into(),
                file_path: manual_path.to_string_lossy().to_string(),
            },
        ]);

        write_log_index(&path, &[auto, manual]).expect("index should write");
        let result = bulk_delete_log_sessions(&path, &log_dir, &active, true)
            .expect("bulk delete should succeed");
        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(result.removed_history_count, 0);
        assert_eq!(result.skipped_active_count, 2);
        assert_eq!(loaded.len(), 2);
        assert!(auto_path.exists());
        assert!(manual_path.exists());
        cleanup(&path);
    }

    #[test]
    fn bulk_delete_removes_old_manual_history_for_same_active_session() {
        let path = temp_index_path();
        let log_dir = path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&log_dir).expect("log dir should be created");
        let old_path = log_dir.join("old_manual.log");
        let active_path = log_dir.join("active_manual.log");
        fs::write(&old_path, "old").expect("old manual file should be created");
        fs::write(&active_path, "active").expect("active manual file should be created");
        let old_manual = LogSession {
            file_path: old_path.to_string_lossy().to_string(),
            log_mode: "manual".into(),
            ..sample_session("active", "2026-04-25T10:00:00+09:00", "host")
        };
        let active_manual = LogSession {
            file_path: active_path.to_string_lossy().to_string(),
            log_mode: "manual".into(),
            ..sample_session("active", "2026-04-25T11:00:00+09:00", "host")
        };
        let active = HashSet::from([ActiveLogKey {
            session_id: "active".into(),
            log_mode: "manual".into(),
            file_path: active_path.to_string_lossy().to_string(),
        }]);

        write_log_index(&path, &[old_manual, active_manual]).expect("index should write");
        let result = bulk_delete_log_sessions(&path, &log_dir, &active, false)
            .expect("bulk delete should succeed");
        let loaded = read_log_index(&path).expect("index should read");

        assert_eq!(result.removed_history_count, 1);
        assert_eq!(result.skipped_active_count, 1);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].file_path,
            active_path.to_string_lossy().to_string()
        );
        cleanup(&path);
    }

    #[test]
    fn bulk_delete_counts_missing_auto_file_as_skipped() {
        let path = temp_index_path();
        let log_dir = path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&log_dir).expect("log dir should be created");
        let auto_path = log_dir.join("missing.log");
        let auto = LogSession {
            file_path: auto_path.to_string_lossy().to_string(),
            ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
        };

        write_log_index(&path, &[auto]).expect("index should write");
        let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), true)
            .expect("bulk delete should succeed");

        assert_eq!(result.removed_history_count, 1);
        assert_eq!(result.skipped_missing_file_count, 1);
        cleanup(&path);
    }

    #[tokio::test]
    async fn bulk_delete_preserves_auto_file_reused_by_manual_logging() {
        for (manual_id, keep_manual_history) in
            [("auto", true), ("manual", true), ("manual", false)]
        {
            let index_path = temp_index_path();
            let dir = index_path.parent().unwrap().to_path_buf();
            let state = LoggerState::with_paths(dir.clone(), index_path.clone());
            let file_path =
                start_log_on_connection(&state, "auto".into(), "ssh".into(), "host".into())
                    .await
                    .unwrap();
            stop_manual_log(&state, "auto").await.unwrap();
            start_manual_log(
                &state,
                manual_id.into(),
                "ssh".into(),
                "host".into(),
                Some(file_path.clone()),
                Some("append".into()),
            )
            .await
            .unwrap();
            let manual = active_log_session(&state, manual_id).await.unwrap();
            append_to_log_sessions(std::slice::from_ref(&manual), "before deletion\n").unwrap();
            if !keep_manual_history {
                let mut history = read_log_index(&index_path).unwrap();
                history.retain(|entry| entry.log_mode == "auto");
                write_log_index(&index_path, &history).unwrap();
            }
            let before = fs::read_to_string(&file_path).unwrap();

            let result = delete_log_sessions(&state, true).await.unwrap();

            let expected_rows = if keep_manual_history { 2 } else { 1 };
            assert_eq!(result.skipped_active_count, expected_rows);
            assert_eq!(result.removed_history_count, 0);
            assert_eq!(result.removed_auto_file_count, 0);
            assert_eq!(read_log_index(&index_path).unwrap().len(), expected_rows);
            append_to_log_sessions(&[manual], "after deletion\n").unwrap();
            assert_eq!(
                fs::read_to_string(file_path).unwrap(),
                format!("{before}after deletion\n")
            );
            cleanup(&index_path);
        }
    }

    #[tokio::test]
    async fn bulk_delete_history_only_removes_old_row_for_shared_active_file() {
        let index_path = temp_index_path();
        let dir = index_path.parent().unwrap().to_path_buf();
        let state = LoggerState::with_paths(dir, index_path.clone());
        let file_path = start_log_on_connection(&state, "auto".into(), "ssh".into(), "host".into())
            .await
            .unwrap();
        stop_manual_log(&state, "auto").await.unwrap();
        start_manual_log(
            &state,
            "manual".into(),
            "ssh".into(),
            "host".into(),
            Some(file_path.clone()),
            Some("append".into()),
        )
        .await
        .unwrap();

        let result = delete_log_sessions(&state, false).await.unwrap();

        assert_eq!(result.removed_history_count, 1);
        assert_eq!(result.skipped_active_count, 1);
        assert_eq!(read_log_index(&index_path).unwrap()[0].log_mode, "manual");
        assert!(Path::new(&file_path).exists());
        cleanup(&index_path);
    }

    #[tokio::test]
    async fn bulk_delete_protects_normalized_path_aliases() {
        let index_path = temp_index_path();
        let dir = index_path.parent().unwrap().to_path_buf();
        let state = LoggerState::with_paths(dir.clone(), index_path.clone());
        let file = dir.join("MixedCase.log");
        start_manual_log(
            &state,
            "manual".into(),
            "ssh".into(),
            "host".into(),
            Some(file.to_string_lossy().into_owned()),
            None,
        )
        .await
        .unwrap();
        let subdir = dir.join("nested");
        fs::create_dir(&subdir).unwrap();
        let aliases = vec![
            dir.join(".").join("MixedCase.log"),
            subdir.join("..").join("MixedCase.log"),
            fs::canonicalize(&file).unwrap(),
        ];
        #[cfg(windows)]
        let aliases = {
            let mut aliases = aliases;
            aliases.push(PathBuf::from(file.to_string_lossy().to_uppercase()));
            aliases.push(PathBuf::from(file.to_string_lossy().replace('\\', "/")));
            aliases
        };
        for alias in aliases {
            let old = LogSession {
                file_path: alias.to_string_lossy().into_owned(),
                ..sample_session("old-auto", "2026-04-25T10:00:00+09:00", "host")
            };
            let active = active_log_session(&state, "manual").await.unwrap();
            write_log_index(&index_path, &[old, active]).unwrap();

            let result = delete_log_sessions(&state, true).await.unwrap();

            assert_eq!(result.skipped_active_count, 2);
            assert_eq!(result.removed_auto_file_count, 0);
            assert!(file.exists());
        }
        cleanup(&index_path);
    }

    #[tokio::test]
    async fn bulk_delete_protects_relative_path_alias() {
        let relative_dir =
            PathBuf::from("target").join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        let dir = std::env::current_dir().unwrap().join(&relative_dir);
        let index_path = dir.join("index.json");
        let state = LoggerState::with_paths(dir.clone(), index_path.clone());
        let file = dir.join("active.log");
        start_manual_log(
            &state,
            "manual".into(),
            "ssh".into(),
            "host".into(),
            Some(file.to_string_lossy().into_owned()),
            None,
        )
        .await
        .unwrap();
        let old = LogSession {
            file_path: relative_dir
                .join("active.log")
                .to_string_lossy()
                .into_owned(),
            ..sample_session("old-auto", "2026-04-25T10:00:00+09:00", "host")
        };
        upsert_log_session(&index_path, old).unwrap();

        let result = delete_log_sessions(&state, true).await.unwrap();

        assert_eq!(result.skipped_active_count, 2);
        assert!(file.exists());
        cleanup(&index_path);
    }

    #[cfg(windows)]
    #[test]
    fn normalized_windows_paths_compare_unicode_without_case() {
        assert!(normalized_paths_equal(
            Path::new(r"C:\logs\Écho.log"),
            Path::new(r"c:\LOGS\éCHO.LOG")
        )
        .unwrap());
        assert!(!normalized_paths_equal(
            Path::new(r"C:\logs\Écho.log"),
            Path::new(r"C:\logs\other.log")
        )
        .unwrap());
    }

    #[tokio::test]
    async fn bulk_delete_fails_before_mutation_if_active_file_cannot_be_resolved() {
        let index_path = temp_index_path();
        let dir = index_path.parent().unwrap().to_path_buf();
        let state = LoggerState::with_paths(dir.clone(), index_path.clone());
        let file_path = start_manual_log(
            &state,
            "manual".into(),
            "ssh".into(),
            "host".into(),
            None,
            None,
        )
        .await
        .unwrap();
        fs::remove_file(file_path).unwrap();
        let inactive_file = dir.join("inactive.log");
        fs::write(&inactive_file, "preserve me").unwrap();
        upsert_log_session(
            &index_path,
            LogSession {
                file_path: inactive_file.to_string_lossy().into_owned(),
                ..sample_session("inactive", "2027-04-25T10:00:00+09:00", "host")
            },
        )
        .unwrap();
        let index_before = fs::read(&index_path).unwrap();

        assert!(delete_log_sessions(&state, true).await.is_err());

        assert_eq!(fs::read(&index_path).unwrap(), index_before);
        assert_eq!(fs::read_to_string(inactive_file).unwrap(), "preserve me");
        assert_eq!(
            delete_log_sessions(&state, false)
                .await
                .unwrap()
                .removed_history_count,
            1
        );
        cleanup(&index_path);
    }

    #[tokio::test]
    async fn bulk_delete_and_log_start_are_serialized() {
        use std::future::Future;
        use std::task::{Context, Poll, Waker};

        let index_path = temp_index_path();
        let dir = index_path.parent().unwrap().to_path_buf();
        let state = LoggerState::with_paths(dir.clone(), index_path.clone());
        let file = dir.join("old.log");
        fs::write(&file, "old content").unwrap();
        upsert_log_session(
            &index_path,
            LogSession {
                file_path: file.to_string_lossy().into_owned(),
                ..sample_session("old-auto", "2026-04-25T10:00:00+09:00", "host")
            },
        )
        .unwrap();
        let guard = state.sessions.lock().await;
        let mut deletion = std::pin::pin!(delete_log_sessions(&state, true));
        let mut start = std::pin::pin!(start_manual_log(
            &state,
            "new-manual".into(),
            "ssh".into(),
            "host".into(),
            Some(file.to_string_lossy().into_owned()),
            Some("append".into()),
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(deletion.as_mut().poll(&mut cx).is_pending());
        assert!(start.as_mut().poll(&mut cx).is_pending());
        drop(guard);

        let Poll::Ready(Ok(result)) = deletion.as_mut().poll(&mut cx) else {
            panic!("deletion should complete before the queued log start");
        };
        assert_eq!(result.removed_auto_file_count, 1);
        assert!(!file.exists());
        assert!(matches!(start.as_mut().poll(&mut cx), Poll::Ready(Ok(_))));
        let active = active_log_session(&state, "new-manual").await.unwrap();
        append_to_log_sessions(&[active], "new content\n").unwrap();
        assert!(fs::read_to_string(&file)
            .unwrap()
            .ends_with("new content\n"));
        let history = read_log_index(&index_path).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].session_id, "new-manual");
        cleanup(&index_path);
    }

    #[test]
    fn bulk_delete_skips_auto_file_outside_log_dir() {
        let path = temp_index_path();
        let log_dir = path.parent().unwrap().to_path_buf();
        fs::create_dir_all(&log_dir).expect("log dir should be created");
        let outside_dir =
            std::env::temp_dir().join(format!("exaterm_logger_outside_{}", Uuid::new_v4()));
        fs::create_dir_all(&outside_dir).expect("outside dir should be created");
        let outside_path = outside_dir.join("auto.log");
        fs::write(&outside_path, "auto").expect("outside file should be created");
        let auto = LogSession {
            file_path: outside_path.to_string_lossy().to_string(),
            ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
        };

        write_log_index(&path, &[auto]).expect("index should write");
        let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), true)
            .expect("bulk delete should succeed");

        assert_eq!(result.removed_history_count, 1);
        assert_eq!(result.skipped_unsafe_path_count, 1);
        assert!(outside_path.exists());
        cleanup(&path);
        let _ = fs::remove_dir_all(outside_dir);
    }
}
