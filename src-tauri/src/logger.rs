use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::Local;

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
    sessions: Arc<Mutex<HashMap<String, LogSession>>>,
}

impl LoggerState {
    pub fn new() -> Self {
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ExaTerm")
            .join("logs");
        let _ = fs::create_dir_all(&log_dir);
        Self { log_dir, sessions: Arc::new(Mutex::new(HashMap::new())) }
    }
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
    let header = format!("# ExaTerm Log\n# Type: {}\n# Target: {}\n# Started: {}\n\n",
        connection_type, target, now.format("%Y-%m-%d %H:%M:%S"));
    fs::write(&file_path, &header).map_err(|e| format!("ログ作成エラー: {}", e))?;

    let session = LogSession {
        session_id: session_id.clone(),
        connection_type, target,
        started_at: now.to_rfc3339(),
        file_path: file_path.to_string_lossy().to_string(),
    };
    state.sessions.lock().await.insert(session_id, session.clone());
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
            .append(true).open(&session.file_path)
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
    let sessions = state.sessions.lock().await;
    Ok(sessions.values().cloned().collect())
}

#[tauri::command]
pub fn logger_get_log_dir(
    state: tauri::State<'_, LoggerState>,
) -> String {
    state.log_dir.to_string_lossy().to_string()
}
