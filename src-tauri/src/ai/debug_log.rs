use chrono::Local;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::types::{AiProvider, ChatMessage};

#[derive(Debug, Clone)]
pub struct ChatDebugLogInput<'a> {
    pub provider: &'a AiProvider,
    pub model: &'a str,
    pub language: &'a str,
    pub terminal_context_included: bool,
    pub messages: &'a [ChatMessage],
    pub system_prompt: &'a str,
    pub duration_ms: u128,
    pub outcome: ChatDebugLogOutcome<'a>,
}

#[derive(Debug, Clone)]
pub enum ChatDebugLogOutcome<'a> {
    Success { response: &'a str },
    Error { error: &'a str },
}

#[derive(Debug, Serialize)]
struct ChatDebugLogRecord<'a> {
    request_id: String,
    timestamp: String,
    provider: &'a str,
    model: &'a str,
    language: &'a str,
    terminal_context_included: bool,
    messages: &'a [ChatMessage],
    system_prompt: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
    duration_ms: u128,
}

pub fn default_ai_debug_log_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join("ai-debug")
}

pub fn append_chat_debug_log(input: ChatDebugLogInput<'_>) -> Result<PathBuf, String> {
    append_chat_debug_log_to_dir(&default_ai_debug_log_dir(), input)
}

fn append_chat_debug_log_to_dir(
    log_dir: &Path,
    input: ChatDebugLogInput<'_>,
) -> Result<PathBuf, String> {
    fs::create_dir_all(log_dir).map_err(|e| format!("AI debug log directory error: {}", e))?;

    let now = Local::now();
    let log_path = log_dir.join(format!("{}.log", now.format("%Y%m%d")));
    let (status, response, error) = match input.outcome {
        ChatDebugLogOutcome::Success { response } => ("success", Some(response), None),
        ChatDebugLogOutcome::Error { error } => ("error", None, Some(error)),
    };
    let record = ChatDebugLogRecord {
        request_id: Uuid::new_v4().to_string(),
        timestamp: now.to_rfc3339(),
        provider: input.provider.id(),
        model: input.model,
        language: input.language,
        terminal_context_included: input.terminal_context_included,
        messages: input.messages,
        system_prompt: input.system_prompt,
        status,
        response,
        error,
        duration_ms: input.duration_ms,
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("AI debug log open error: {}", e))?;
    serde_json::to_writer(&mut file, &record)
        .map_err(|e| format!("AI debug log serialize error: {}", e))?;
    writeln!(file).map_err(|e| format!("AI debug log write error: {}", e))?;

    Ok(log_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log_dir() -> PathBuf {
        std::env::temp_dir().join(format!("exaterm_ai_debug_log_test_{}", Uuid::new_v4()))
    }

    fn sample_input<'a>(
        outcome: ChatDebugLogOutcome<'a>,
        messages: &'a [ChatMessage],
    ) -> ChatDebugLogInput<'a> {
        ChatDebugLogInput {
            provider: &AiProvider::OpenAi,
            model: "gpt-4o",
            language: "en",
            terminal_context_included: true,
            messages,
            system_prompt: "system\nprompt",
            duration_ms: 42,
            outcome,
        }
    }

    #[test]
    fn writes_success_record_as_json_line() {
        let dir = temp_log_dir();
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "hello\nworld".into(),
        }];

        let path = append_chat_debug_log_to_dir(
            &dir,
            sample_input(ChatDebugLogOutcome::Success { response: "ok" }, &messages),
        )
        .expect("debug log should write");
        let data = fs::read_to_string(&path).expect("debug log should read");
        let lines = data.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 1);
        let value: serde_json::Value =
            serde_json::from_str(lines[0]).expect("debug log line should be json");
        assert_eq!(value["provider"], "OpenAi");
        assert_eq!(value["model"], "gpt-4o");
        assert_eq!(value["status"], "success");
        assert_eq!(value["response"], "ok");
        assert_eq!(value["messages"][0]["content"], "hello\nworld");
        assert_eq!(value["system_prompt"], "system\nprompt");
        assert!(value.get("api_key").is_none());
        assert!(value.get("headers").is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn appends_error_record_to_same_daily_file() {
        let dir = temp_log_dir();
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "first".into(),
        }];

        let first_path = append_chat_debug_log_to_dir(
            &dir,
            sample_input(ChatDebugLogOutcome::Success { response: "ok" }, &messages),
        )
        .expect("first debug log should write");
        let second_path = append_chat_debug_log_to_dir(
            &dir,
            sample_input(
                ChatDebugLogOutcome::Error {
                    error: "network error",
                },
                &messages,
            ),
        )
        .expect("second debug log should write");

        assert_eq!(first_path, second_path);
        let data = fs::read_to_string(&first_path).expect("debug log should read");
        let lines = data.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        let value: serde_json::Value =
            serde_json::from_str(lines[1]).expect("debug log line should be json");
        assert_eq!(value["status"], "error");
        assert_eq!(value["error"], "network error");
        assert!(value.get("response").is_none());
        assert!(value.get("authorization").is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
