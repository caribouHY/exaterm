use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackendCommandError {
    pub code: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value>,
    pub message: String,
}

impl std::fmt::Display for BackendCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BackendCommandError {}

impl BackendCommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            params: BTreeMap::new(),
            message: message.into(),
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        if let Some(error) = ai_error(&message) {
            return error;
        }
        if let Some((code, param, prefix)) = prefixed_error(&message) {
            return Self::new(code, &message)
                .with_param(param, message[prefix.len()..].to_string());
        }

        let code = match message.as_str() {
            "Session not found" => "terminal.session_not_found",
            "The specified cursor is past the current output position" => {
                "terminal.cursor_out_of_range"
            }
            "Window not found" => "workspace.window_not_found",
            "Source window not found" => "workspace.source_window_not_found",
            "Destination window not found" => "workspace.destination_window_not_found",
            "Tab not found" => "workspace.tab_not_found",
            "Source tab not found" => "workspace.source_tab_not_found",
            "Destination tab not found" => "workspace.destination_tab_not_found",
            "The tab does not belong to this window" => "workspace.tab_not_in_window",
            "The tab owner window does not match" => "workspace.tab_owner_mismatch",
            "No tab is currently being dragged" => "workspace.no_active_drag",
            "Destination snapshot not found" => "workspace.destination_snapshot_not_found",
            "The saved SSH host key does not match" => "ssh.host_key_mismatch",
            "The SSH connection attempt was cancelled" => "ssh.connect_cancelled",
            "The SSH authentication method is invalid" => "ssh.invalid_auth_method",
            "The SSH authentication prompt was cancelled" => "ssh.auth_prompt_cancelled",
            "The SSH authentication prompt timed out" => "ssh.auth_prompt_timed_out",
            "The SSH authentication response count does not match the prompt count" => {
                "ssh.auth_prompt_response_mismatch"
            }
            "The SSH host key prompt was cancelled" => "ssh.host_key_prompt_cancelled",
            "The SSH host key prompt timed out" => "ssh.host_key_prompt_timed_out",
            "The SSH host key prompt request was not found" => {
                "ssh.host_key_prompt_request_not_found"
            }
            "The SSH host key prompt request has already finished" => {
                "ssh.host_key_prompt_already_finished"
            }
            "Failed to retrieve the SSH host key" => "ssh.host_key_retrieval_failed",
            "An SSH jump profile cannot reference itself" => "ssh.jump_profile_self_reference",
            "SSH jump profile not found" => "ssh.jump_profile_not_found",
            "Only SSH profiles can be used as SSH jump profiles" => "ssh.jump_profile_wrong_type",
            "Nested SSH jump profiles are not supported" => "ssh.jump_profile_nested",
            "The SSH jump profile does not have a host configured" => {
                "ssh.jump_profile_host_missing"
            }
            "The SSH jump profile does not have a username configured" => {
                "ssh.jump_profile_username_missing"
            }
            "The SSH jump profile does not have a private key file configured" => {
                "ssh.jump_profile_key_missing"
            }
            "Specify a private key file" => "ssh.private_key_required",
            "PTY request failed" => "ssh.pty_request_failed",
            "Shell request failed" => "ssh.shell_request_failed",
            "Secret value cannot be empty" => "ai.secret_empty",
            "Unsupported provider for secure secret storage" => "ai.unsupported_secret_provider",
            "No AI model is selected. Choose a model, then try again." => "ai.model_not_selected",
            "Configure the Azure OpenAI endpoint and model deployment name in Settings." => {
                "ai.azure_endpoint_missing"
            }
            "The Azure OpenAI endpoint URL is invalid. Check the value in Settings." => {
                "ai.azure_endpoint_invalid"
            }
            _ => "backend.operation_failed",
        };
        Self::new(code, message)
    }
}

fn ai_error(message: &str) -> Option<BackendCommandError> {
    if let Some(provider) = message
        .strip_suffix(" API key is not configured. Save the API key in Settings, then try again.")
    {
        return Some(
            BackendCommandError::new("ai.secret_missing", message).with_param("provider", provider),
        );
    }
    if let Some(detail) = message.strip_prefix(
        "Could not connect to Ollama. Check that Ollama is running and the Base URL is correct. Details: ",
    ) {
        return Some(
            BackendCommandError::new("ai.ollama_unavailable", message)
                .with_param("detail", detail),
        );
    }
    if let Some(rest) = message.strip_prefix("Could not connect to ") {
        if let Some((provider, detail)) = rest
            .split_once(". Check your network connection, proxy, and firewall settings. Details: ")
        {
            return Some(
                BackendCommandError::new("ai.provider_unavailable", message)
                    .with_param("provider", provider)
                    .with_param("detail", detail),
            );
        }
    }

    let (base, provider_detail) = message
        .split_once(" Provider message: ")
        .map(|(base, detail)| (base, Some(detail)))
        .unwrap_or((message, None));
    for (suffix, code) in [
        (
            " authentication failed. Check that the API key is correct and has the required permissions.",
            "ai.authentication_failed",
        ),
        (
            " endpoint or model was not found. Check that the selected model is currently available.",
            "ai.model_not_found",
        ),
        (
            " rate limit or quota was reached. Check your usage limits or try again later.",
            "ai.quota_exceeded",
        ),
        (
            " is currently returning a server error. Try again later.",
            "ai.server_error",
        ),
    ] {
        if let Some(provider) = base.strip_suffix(suffix) {
            return Some(
                BackendCommandError::new(code, message)
                    .with_param("provider", provider)
                    .with_param("detail", provider_detail.unwrap_or("")),
            );
        }
    }

    if let Some((provider, status_and_tail)) = base.split_once(" returned HTTP ") {
        if let Some(status) =
            status_and_tail.strip_suffix(". Check your settings and selected model.")
        {
            let error = BackendCommandError::new("ai.http_error", message)
                .with_param("provider", provider)
                .with_param("status", status)
                .with_param("detail", provider_detail.unwrap_or(""));
            return Some(error);
        }
    }
    None
}

impl From<String> for BackendCommandError {
    fn from(message: String) -> Self {
        Self::from_message(message)
    }
}

impl From<&str> for BackendCommandError {
    fn from(message: &str) -> Self {
        Self::from_message(message)
    }
}

fn prefixed_error(message: &str) -> Option<(&'static str, &'static str, &'static str)> {
    [
        (
            "logger.history_read_failed",
            "detail",
            "Failed to read log history: ",
        ),
        (
            "logger.history_parse_failed",
            "detail",
            "Failed to parse log history: ",
        ),
        (
            "logger.directory_create_failed",
            "detail",
            "Failed to create the log directory: ",
        ),
        (
            "logger.file_create_failed",
            "detail",
            "Failed to create the log file: ",
        ),
        (
            "logger.file_write_failed",
            "detail",
            "Failed to write to the log file: ",
        ),
        (
            "logger.file_delete_failed",
            "detail",
            "Failed to delete the log file: ",
        ),
        (
            "workspace.window_create_failed",
            "detail",
            "Failed to create the window: ",
        ),
        (
            "serial.list_ports_failed",
            "detail",
            "Failed to list serial ports: ",
        ),
        (
            "serial.open_failed",
            "detail",
            "Failed to open the serial port: ",
        ),
        (
            "serial.clone_failed",
            "detail",
            "Failed to clone the serial port handle: ",
        ),
        ("terminal.send_failed", "detail", "Failed to send data: "),
        (
            "telnet.connect_failed",
            "detail",
            "Failed to connect over Telnet: ",
        ),
        (
            "telnet.resize_failed",
            "detail",
            "Failed to send the resize request: ",
        ),
        (
            "ssh.channel_open_failed",
            "detail",
            "Failed to open the SSH channel: ",
        ),
        (
            "ssh.jump_channel_open_failed",
            "detail",
            "Failed to open the SSH jump channel: ",
        ),
        ("ssh.connection_failed", "detail", "SSH connection error: "),
        (
            "ssh.public_key_auth_failed",
            "detail",
            "SSH public key authentication error: ",
        ),
        (
            "ssh.authentication_failed",
            "detail",
            "SSH authentication error: ",
        ),
        (
            "ssh.private_key_open_failed",
            "detail",
            "Failed to open the private key file: ",
        ),
        (
            "ssh.private_key_load_failed",
            "detail",
            "Failed to load the private key: ",
        ),
        (
            "config.load_failed",
            "detail",
            "Failed to load the configuration: ",
        ),
        (
            "connection.unknown_type",
            "detail",
            "Unknown connection type: ",
        ),
    ]
    .into_iter()
    .find(|(_, _, prefix)| message.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_stable_command_error_contract() {
        let error = BackendCommandError::new("terminal.session_not_found", "Session not found.")
            .with_param("attempt", 2);
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            serde_json::json!({
                "code": "terminal.session_not_found",
                "params": { "attempt": 2 },
                "message": "Session not found."
            })
        );
    }

    #[test]
    fn classifies_prefixed_errors_without_localizing_the_fallback() {
        let error = BackendCommandError::from_message("Failed to connect over Telnet: refused");
        assert_eq!(error.code, "telnet.connect_failed");
        assert_eq!(error.params["detail"], "refused");
        assert_eq!(error.message, "Failed to connect over Telnet: refused");
    }

    #[test]
    fn classifies_ssh_authentication_prompt_errors() {
        assert_eq!(
            BackendCommandError::from_message("The SSH authentication prompt was cancelled").code,
            "ssh.auth_prompt_cancelled"
        );
        assert_eq!(
            BackendCommandError::from_message("The SSH authentication prompt timed out").code,
            "ssh.auth_prompt_timed_out"
        );
        assert_eq!(
            BackendCommandError::from_message(
                "The SSH authentication response count does not match the prompt count"
            )
            .code,
            "ssh.auth_prompt_response_mismatch"
        );
    }

    #[test]
    fn classifies_ssh_connection_cancellation() {
        assert_eq!(
            BackendCommandError::from_message("The SSH connection attempt was cancelled").code,
            "ssh.connect_cancelled"
        );
    }

    #[test]
    fn classifies_ssh_host_key_prompt_errors() {
        assert_eq!(
            BackendCommandError::from_message("The SSH host key prompt was cancelled").code,
            "ssh.host_key_prompt_cancelled"
        );
        assert_eq!(
            BackendCommandError::from_message("The SSH host key prompt timed out").code,
            "ssh.host_key_prompt_timed_out"
        );
        assert_eq!(
            BackendCommandError::from_message("The SSH host key prompt request was not found").code,
            "ssh.host_key_prompt_request_not_found"
        );
        assert_eq!(
            BackendCommandError::from_message(
                "The SSH host key prompt request has already finished"
            )
            .code,
            "ssh.host_key_prompt_already_finished"
        );
    }

    #[test]
    fn classifies_ai_provider_errors_without_using_response_language() {
        let error = BackendCommandError::from_message(
            "OpenAI authentication failed. Check that the API key is correct and has the required permissions. Provider message: denied",
        );
        assert_eq!(error.code, "ai.authentication_failed");
        assert_eq!(error.params["provider"], "OpenAI");
        assert_eq!(error.params["detail"], "denied");
        assert!(error.message.starts_with("OpenAI authentication failed"));
    }
}
