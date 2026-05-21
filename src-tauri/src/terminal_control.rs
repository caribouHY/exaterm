use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{futures::Notified, Mutex, Notify};

const DEFAULT_OUTPUT_LIMIT: usize = 64 * 1024;
const DEFAULT_SNAPSHOT_MAX_CHARS: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalProtocol {
    Ssh,
    Serial,
    Telnet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStatus {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalSessionInfo {
    pub session_id: String,
    pub protocol: TerminalProtocol,
    pub target: String,
    pub status: TerminalStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalOutputSnapshot {
    pub session_id: String,
    pub output: String,
    pub truncated: bool,
    pub available_chars: usize,
    pub start_cursor: usize,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
struct TerminalSession {
    info: TerminalSessionInfo,
    output: String,
    dropped_chars: usize,
}

#[derive(Clone)]
pub struct TerminalControlState {
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
    output_notify: Arc<Notify>,
    output_limit: usize,
}

impl TerminalControlState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            output_notify: Arc::new(Notify::new()),
            output_limit: DEFAULT_OUTPUT_LIMIT,
        }
    }

    #[cfg(test)]
    fn with_output_limit(output_limit: usize) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            output_notify: Arc::new(Notify::new()),
            output_limit,
        }
    }

    pub async fn register_session(
        &self,
        session_id: String,
        protocol: TerminalProtocol,
        target: String,
    ) {
        let info = TerminalSessionInfo {
            session_id: session_id.clone(),
            protocol,
            target,
            status: TerminalStatus::Connected,
        };
        let session = TerminalSession {
            info,
            output: String::new(),
            dropped_chars: 0,
        };

        self.sessions.lock().await.insert(session_id, session);
    }

    pub async fn append_output(&self, session_id: &str, data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.get_mut(session_id) else {
            return;
        };

        session.output.push_str(&text);
        trim_to_recent_chars(session, self.output_limit);
        drop(sessions);
        self.output_notify.notify_waiters();
    }

    pub async fn mark_disconnected(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.info.status = TerminalStatus::Disconnected;
        }
    }

    pub async fn list_sessions(&self) -> Vec<TerminalSessionInfo> {
        let mut sessions = self
            .sessions
            .lock()
            .await
            .values()
            .map(|session| session.info.clone())
            .collect::<Vec<_>>();
        sessions.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        sessions
    }

    pub async fn session_info(&self, session_id: &str) -> Option<TerminalSessionInfo> {
        self.sessions
            .lock()
            .await
            .get(session_id)
            .map(|session| session.info.clone())
    }

    pub async fn read_output(
        &self,
        session_id: &str,
        max_chars: usize,
    ) -> Result<TerminalOutputSnapshot, String> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| "セッションが見つかりません".to_string())?;
        let available_chars = session.output.chars().count();
        let requested_chars = max_chars.max(1);
        let output = tail_chars(&session.output, requested_chars);
        let output_chars = output.chars().count();
        let truncated = session.dropped_chars > 0 || output_chars < available_chars;
        let end_cursor = session.dropped_chars + available_chars;

        Ok(TerminalOutputSnapshot {
            session_id: session_id.to_string(),
            output,
            truncated,
            available_chars,
            start_cursor: end_cursor - output_chars,
            cursor: end_cursor,
        })
    }

    pub async fn cursor(&self, session_id: &str) -> Result<usize, String> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| "セッションが見つかりません".to_string())?;
        Ok(session.dropped_chars + session.output.chars().count())
    }

    pub async fn read_output_delta(
        &self,
        session_id: &str,
        cursor: usize,
        max_chars: usize,
    ) -> Result<TerminalOutputSnapshot, String> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| "セッションが見つかりません".to_string())?;
        let available_chars = session.output.chars().count();
        let start_cursor = session.dropped_chars;
        let end_cursor = start_cursor + available_chars;

        if cursor > end_cursor {
            return Err("指定されたカーソルは現在の出力位置より先です".to_string());
        }

        let requested_chars = max_chars.max(1);
        let effective_cursor = cursor.max(start_cursor);
        let relative_start = effective_cursor - start_cursor;
        let delta = chars_from(&session.output, relative_start);
        let delta_chars = delta.chars().count();
        let output = tail_chars(&delta, requested_chars);
        let output_chars = output.chars().count();
        let returned_start_cursor = end_cursor - output_chars;
        let truncated = cursor < start_cursor || output_chars < delta_chars;

        Ok(TerminalOutputSnapshot {
            session_id: session_id.to_string(),
            output,
            truncated,
            available_chars,
            start_cursor: returned_start_cursor,
            cursor: end_cursor,
        })
    }

    pub fn output_change_notified(&self) -> Notified<'_> {
        self.output_notify.notified()
    }
}

fn trim_to_recent_chars(session: &mut TerminalSession, output_limit: usize) {
    let char_count = session.output.chars().count();
    if char_count <= output_limit {
        return;
    }

    let keep_from = char_count - output_limit;
    let byte_index = session
        .output
        .char_indices()
        .nth(keep_from)
        .map(|(idx, _)| idx)
        .unwrap_or(session.output.len());
    session.output.drain(..byte_index);
    session.dropped_chars += keep_from;
}

fn tail_chars(input: &str, max_chars: usize) -> String {
    let char_count = input.chars().count();
    if char_count <= max_chars {
        return input.to_string();
    }

    input.chars().skip(char_count - max_chars).collect()
}

fn chars_from(input: &str, start: usize) -> String {
    input.chars().skip(start).collect()
}

#[tauri::command]
pub async fn terminal_output_snapshot_get(
    state: tauri::State<'_, TerminalControlState>,
    session_id: String,
    max_chars: Option<usize>,
) -> Result<TerminalOutputSnapshot, String> {
    state
        .read_output(
            &session_id,
            max_chars.unwrap_or(DEFAULT_SNAPSHOT_MAX_CHARS).max(1),
        )
        .await
}

#[tauri::command]
pub async fn terminal_output_delta_get(
    state: tauri::State<'_, TerminalControlState>,
    session_id: String,
    cursor: usize,
    max_chars: Option<usize>,
) -> Result<TerminalOutputSnapshot, String> {
    state
        .read_output_delta(
            &session_id,
            cursor,
            max_chars.unwrap_or(DEFAULT_SNAPSHOT_MAX_CHARS).max(1),
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn output_is_trimmed_to_recent_chars() {
        let state = TerminalControlState::with_output_limit(5);
        state
            .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
            .await;

        state.append_output("s1", "abcdef".as_bytes()).await;

        let snapshot = state.read_output("s1", 10).await.unwrap();
        assert_eq!(snapshot.output, "bcdef");
        assert!(snapshot.truncated);
        assert_eq!(snapshot.available_chars, 5);
        assert_eq!(snapshot.start_cursor, 1);
        assert_eq!(snapshot.cursor, 6);
    }

    #[tokio::test]
    async fn read_output_can_return_tail_subset() {
        let state = TerminalControlState::with_output_limit(20);
        state
            .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
            .await;

        state.append_output("s1", "こんにちは世界".as_bytes()).await;

        let snapshot = state.read_output("s1", 2).await.unwrap();
        assert_eq!(snapshot.output, "世界");
        assert!(snapshot.truncated);
        assert_eq!(snapshot.start_cursor, 5);
        assert_eq!(snapshot.cursor, 7);
    }

    #[tokio::test]
    async fn disconnected_sessions_remain_readable() {
        let state = TerminalControlState::new();
        state
            .register_session("s1".into(), TerminalProtocol::Telnet, "host:23".into())
            .await;
        state.append_output("s1", b"login: ").await;
        state.mark_disconnected("s1").await;

        let sessions = state.list_sessions().await;
        assert_eq!(sessions[0].status, TerminalStatus::Disconnected);
        assert_eq!(
            state.read_output("s1", 100).await.unwrap().output,
            "login: "
        );
    }

    #[tokio::test]
    async fn read_output_delta_returns_text_after_cursor() {
        let state = TerminalControlState::new();
        state
            .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
            .await;
        state.append_output("s1", "abcこんにちは".as_bytes()).await;

        let snapshot = state.read_output_delta("s1", 3, 100).await.unwrap();

        assert_eq!(snapshot.output, "こんにちは");
        assert!(!snapshot.truncated);
        assert_eq!(snapshot.start_cursor, 3);
        assert_eq!(snapshot.cursor, 8);
    }

    #[tokio::test]
    async fn read_output_delta_reports_truncation_for_old_cursor() {
        let state = TerminalControlState::with_output_limit(5);
        state
            .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
            .await;
        state.append_output("s1", "abcdef".as_bytes()).await;

        let snapshot = state.read_output_delta("s1", 0, 100).await.unwrap();

        assert_eq!(snapshot.output, "bcdef");
        assert!(snapshot.truncated);
        assert_eq!(snapshot.start_cursor, 1);
        assert_eq!(snapshot.cursor, 6);
    }

    #[tokio::test]
    async fn read_output_delta_is_not_truncated_when_requested_cursor_is_retained() {
        let state = TerminalControlState::with_output_limit(5);
        state
            .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
            .await;
        state.append_output("s1", "abcdef".as_bytes()).await;

        let snapshot = state.read_output_delta("s1", 3, 100).await.unwrap();

        assert_eq!(snapshot.output, "def");
        assert!(!snapshot.truncated);
        assert_eq!(snapshot.start_cursor, 3);
        assert_eq!(snapshot.cursor, 6);
    }

    #[tokio::test]
    async fn read_output_delta_rejects_future_cursor() {
        let state = TerminalControlState::new();
        state
            .register_session("s1".into(), TerminalProtocol::Telnet, "host:23".into())
            .await;

        let error = state.read_output_delta("s1", 1, 100).await.unwrap_err();

        assert!(error.contains("カーソル"));
    }
}
