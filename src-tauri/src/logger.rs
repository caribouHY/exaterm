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
    pub connection_type: String, // "ssh" or "serial"
    pub target: String,          // e.g. "user@host:22" or "COM3"
    pub started_at: String,
    pub file_path: String,
}

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
        .find(|item| item.session_id == session.session_id)
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

#[tauri::command]
pub async fn logger_start(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    connection_type: String,
    target: String,
) -> Result<String, String> {
    let now = Local::now();
    let filename = format!("{}_{}.log", now.format("%Y%m%d_%H%M%S"), &session_id[..8]);
    let file_path = state.log_dir.join(&filename);
    let header = format!(
        "# ExaTerm Log\n# Type: {}\n# Target: {}\n# Started: {}\n\n",
        connection_type,
        target,
        now.format("%Y-%m-%d %H:%M:%S")
    );
    fs::write(&file_path, &header).map_err(|e| format!("ログ作成エラー: {}", e))?;

    let session = LogSession {
        session_id: session_id.clone(),
        connection_type,
        target,
        started_at: now.to_rfc3339(),
        file_path: file_path.to_string_lossy().to_string(),
    };
    let mut sessions = state.sessions.lock().await;
    sessions.insert(session_id, session.clone());
    upsert_log_session(&state.index_path, session.clone())?;
    Ok(session.file_path)
}

#[tauri::command]
pub async fn logger_append(
    state: tauri::State<'_, LoggerState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    if let Some(session) = sessions.get(&session_id) {
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
