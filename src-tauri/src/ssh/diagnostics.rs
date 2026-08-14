use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::ssh::host_key::HostKeyVerifier;
use crate::ssh_known_hosts::{HostKeyCheckResult, HostKeyCheckStatus};

#[derive(Clone)]
pub(super) struct SshDiagnostic {
    app: AppHandle,
    request_id: Option<String>,
    window_id: String,
}

#[derive(Clone, Debug, Serialize)]
struct SshDiagnosticEvent {
    level: &'static str,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
struct SshConnectionProgressEvent {
    phase: &'static str,
    target: &'static str,
}

pub(super) fn ssh_diagnostic_event_name(request_id: &str) -> String {
    format!("ssh://connect-diagnostic/{request_id}")
}

pub(super) fn ssh_progress_event_name(request_id: &str) -> String {
    format!("ssh://connect-progress/{request_id}")
}

fn emit_ssh_diagnostic(
    app: &AppHandle,
    window_id: &str,
    request_id: Option<&str>,
    level: &'static str,
    message: impl Into<String>,
) {
    let Some(request_id) = request_id else {
        return;
    };

    let _ = app.emit_to(
        window_id,
        &ssh_diagnostic_event_name(request_id),
        SshDiagnosticEvent {
            level,
            message: message.into(),
        },
    );
}

impl SshDiagnostic {
    pub(super) fn new(app: &AppHandle, request_id: Option<String>, window_id: String) -> Self {
        Self {
            app: app.clone(),
            request_id,
            window_id,
        }
    }

    pub(super) fn info(&self, message: impl Into<String>) {
        emit_ssh_diagnostic(
            &self.app,
            &self.window_id,
            self.request_id.as_deref(),
            "info",
            message,
        );
    }

    pub(super) fn error(&self, message: impl Into<String>) {
        emit_ssh_diagnostic(
            &self.app,
            &self.window_id,
            self.request_id.as_deref(),
            "error",
            message,
        );
    }

    pub(super) fn progress(&self, target: &'static str, phase: &'static str) {
        let Some(request_id) = self.request_id.as_deref() else {
            return;
        };
        let _ = self.app.emit_to(
            &self.window_id,
            &ssh_progress_event_name(request_id),
            SshConnectionProgressEvent { phase, target },
        );
    }
}

/// Global SSH session store

pub(super) fn host_key_error_message(result: &HostKeyCheckResult) -> String {
    match result.status {
        HostKeyCheckStatus::Unknown => format!(
            "The SSH host key is untrusted. Verify the fingerprint before connecting: SHA256:{}",
            result.fingerprint
        ),
        HostKeyCheckStatus::Mismatch => format!(
            "The SSH host key does not match. A MITM attack may be in progress. Saved: {} / Received: SHA256:{}",
            result
                .known_fingerprint
                .as_deref()
                .map(|fingerprint| format!("SHA256:{}", fingerprint))
                .unwrap_or_else(|| "unknown".to_string()),
            result.fingerprint
        ),
        HostKeyCheckStatus::Trusted => "SSH host key verification error".to_string(),
    }
}

pub(super) fn map_connect_error(error: russh::Error, verifier: &HostKeyVerifier) -> String {
    if let Some(error) = verifier.last_error() {
        return error;
    }
    if let Some(result) = verifier.last_result() {
        if result.status != HostKeyCheckStatus::Trusted {
            return host_key_error_message(&result);
        }
    }
    format!("SSH connection error: {}", error)
}

pub(super) fn emit_host_key_diagnostic_for_role(
    diagnostic: &SshDiagnostic,
    role: &str,
    result: &HostKeyCheckResult,
) {
    diagnostic.info(format!(
        "{role}: host key received {} SHA256:{}",
        result.algorithm, result.fingerprint
    ));
    match result.status {
        HostKeyCheckStatus::Trusted => diagnostic.info(format!("{role}: host key trusted")),
        HostKeyCheckStatus::Unknown => diagnostic.info(format!("{role}: host key unknown")),
        HostKeyCheckStatus::Mismatch => diagnostic.info(format!("{role}: host key mismatch")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_payload_uses_the_public_event_values() {
        let payload = serde_json::to_value(SshConnectionProgressEvent {
            phase: "verifying_host_key",
            target: "jump",
        })
        .unwrap();

        assert_eq!(payload["phase"], "verifying_host_key");
        assert_eq!(payload["target"], "jump");
    }
}
