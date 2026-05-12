use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_OUTPUT_LIMIT: usize = 64 * 1024;

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
    output_limit: usize,
}

impl TerminalControlState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            output_limit: DEFAULT_OUTPUT_LIMIT,
        }
    }

    #[cfg(test)]
    fn with_output_limit(output_limit: usize) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
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
        let truncated = session.dropped_chars > 0 || output.chars().count() < available_chars;

        Ok(TerminalOutputSnapshot {
            session_id: session_id.to_string(),
            output,
            truncated,
            available_chars,
        })
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
}
