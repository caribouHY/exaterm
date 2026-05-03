use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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

#[derive(Debug, Clone, Default)]
struct ActiveLogTargets {
    auto: Option<LogSession>,
    manual: Option<LogSession>,
}

pub struct LoggerState {
    log_dir: PathBuf,
    index_path: PathBuf,
    sessions: Arc<Mutex<HashMap<String, ActiveLogTargets>>>,
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
}

fn default_log_mode() -> String {
    "auto".into()
}

fn read_log_index(index_path: &PathBuf) -> Result<Vec<LogSession>, String> {
    if !index_path.exists() {
        return Ok(Vec::new());
    }

    let data =
        fs::read_to_string(index_path).map_err(|e| format!("ログ履歴読み込みエラー: {}", e))?;
    let mut sessions: Vec<LogSession> =
        serde_json::from_str(&data).map_err(|e| format!("ログ履歴解析エラー: {}", e))?;
    sort_sessions_desc(&mut sessions);
    Ok(sessions)
}

fn write_log_index(index_path: &PathBuf, sessions: &[LogSession]) -> Result<(), String> {
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("ログ履歴ディレクトリ作成エラー: {}", e))?;
    }

    let data = serde_json::to_string_pretty(sessions)
        .map_err(|e| format!("ログ履歴シリアライズエラー: {}", e))?;
    fs::write(index_path, data).map_err(|e| format!("ログ履歴保存エラー: {}", e))
}

fn upsert_log_session(index_path: &PathBuf, session: LogSession) -> Result<(), String> {
    let mut sessions = read_log_index(index_path)?;
    if let Some(existing) = sessions
        .iter_mut()
        .find(|item| item.session_id == session.session_id && item.log_mode == session.log_mode)
    {
        *existing = session;
    } else {
        sessions.push(session);
    }
    sort_sessions_desc(&mut sessions);
    write_log_index(index_path, &sessions)
}

fn sort_sessions_desc(sessions: &mut [LogSession]) {
    sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));
}

fn create_log_session(
    log_dir: &PathBuf,
    session_id: String,
    connection_type: String,
    target: String,
    file_path: Option<String>,
    log_mode: &str,
    include_header: bool,
) -> Result<LogSession, String> {
    let now = Local::now();
    let session_prefix = session_id.chars().take(8).collect::<String>();
    let filename = format!("{}_{}.log", now.format("%Y%m%d_%H%M%S"), session_prefix);
    let file_path = file_path
        .map(PathBuf::from)
        .unwrap_or_else(|| log_dir.join(&filename));
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("ログディレクトリ作成エラー: {}", e))?;
    }
    if include_header {
        let header = format!(
            "# ExaTerm Log\n# Type: {}\n# Target: {}\n# Mode: {}\n# Started: {}\n\n",
            connection_type,
            target,
            log_mode,
            now.format("%Y-%m-%d %H:%M:%S")
        );
        fs::write(&file_path, &header).map_err(|e| format!("ログ作成エラー: {}", e))?;
    } else {
        fs::write(&file_path, "").map_err(|e| format!("ログ作成エラー: {}", e))?;
    }

    Ok(LogSession {
        session_id: session_id.clone(),
        connection_type,
        target,
        started_at: now.to_rfc3339(),
        file_path: file_path.to_string_lossy().to_string(),
        log_mode: log_mode.into(),
    })
}

fn append_to_log_sessions(sessions: &[LogSession], data: &str) -> Result<(), String> {
    for session in sessions {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&session.file_path)
            .map_err(|e| format!("ログ書き込みエラー: {}", e))?;
        use std::io::Write;
        write!(file, "{}", data).map_err(|e| format!("ログ書き込みエラー: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn logger_start_auto(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    connection_type: String,
    target: String,
) -> Result<String, String> {
    let include_header = crate::config::config_read()
        .map(|cfg| cfg.terminal.include_log_header)
        .unwrap_or(true);
    let session = create_log_session(
        &state.log_dir,
        session_id.clone(),
        connection_type,
        target,
        None,
        "auto",
        include_header,
    )?;
    let mut sessions = state.sessions.lock().await;
    sessions.entry(session_id).or_default().auto = Some(session.clone());
    upsert_log_session(&state.index_path, session.clone())?;
    Ok(session.file_path)
}

#[tauri::command]
pub async fn logger_start_manual(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    connection_type: String,
    target: String,
    file_path: String,
) -> Result<String, String> {
    let include_header = crate::config::config_read()
        .map(|cfg| cfg.terminal.include_log_header)
        .unwrap_or(true);
    let session = create_log_session(
        &state.log_dir,
        session_id.clone(),
        connection_type,
        target,
        Some(file_path),
        "manual",
        include_header,
    )?;
    let mut sessions = state.sessions.lock().await;
    sessions.entry(session_id).or_default().manual = Some(session.clone());
    upsert_log_session(&state.index_path, session.clone())?;
    Ok(session.file_path)
}

#[tauri::command]
pub async fn logger_start(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    connection_type: String,
    target: String,
) -> Result<String, String> {
    logger_start_auto(state, session_id, connection_type, target).await
}

#[tauri::command]
pub async fn logger_stop_manual(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    if let Some(targets) = sessions.get_mut(&session_id) {
        targets.manual = None;
        if targets.auto.is_none() {
            sessions.remove(&session_id);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn logger_is_manual_active(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<bool, String> {
    let sessions = state.sessions.lock().await;
    Ok(sessions
        .get(&session_id)
        .and_then(|targets| targets.manual.as_ref())
        .is_some())
}

#[tauri::command]
pub async fn logger_append(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let active_sessions = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(&session_id)
            .map(|targets| {
                targets
                    .auto
                    .iter()
                    .chain(targets.manual.iter())
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    append_to_log_sessions(&active_sessions, &data)?;
    Ok(())
}

#[tauri::command]
pub async fn logger_get_sessions(
    state: tauri::State<'_, LoggerState>,
) -> Result<Vec<LogSession>, String> {
    let _sessions = state.sessions.lock().await;
    read_log_index(&state.index_path)
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
    fn append_to_log_sessions_writes_to_all_targets() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let auto_path = dir.join("auto.log");
        let manual_path = dir.join("manual.log");
        fs::write(&auto_path, "auto\n").expect("auto file should be created");
        fs::write(&manual_path, "manual\n").expect("manual file should be created");

        let sessions = vec![
            LogSession {
                file_path: auto_path.to_string_lossy().to_string(),
                ..sample_session("session-1", "2026-04-25T10:00:00+09:00", "host")
            },
            LogSession {
                file_path: manual_path.to_string_lossy().to_string(),
                log_mode: "manual".into(),
                ..sample_session("session-1", "2026-04-25T10:00:00+09:00", "host")
            },
        ];

        append_to_log_sessions(&sessions, "data\n").expect("append should write both logs");

        assert_eq!(fs::read_to_string(&auto_path).unwrap(), "auto\ndata\n");
        assert_eq!(fs::read_to_string(&manual_path).unwrap(), "manual\ndata\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_log_session_writes_header_when_enabled() {
        let dir = std::env::temp_dir().join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir should be created");

        let session = create_log_session(
            &dir,
            "session-1".into(),
            "ssh".into(),
            "user@host:22".into(),
            None,
            "auto",
            true,
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
            "session-1".into(),
            "ssh".into(),
            "user@host:22".into(),
            None,
            "auto",
            false,
        )
        .expect("log session should be created");
        let data = fs::read_to_string(&session.file_path).expect("log should read");

        assert_eq!(data, "");
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
}
