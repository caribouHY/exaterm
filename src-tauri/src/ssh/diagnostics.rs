use russh::keys::PublicKey;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::ssh::host_key::HostKeyVerifier;
use crate::ssh_known_hosts::{HostKeyCheckResult, HostKeyCheckStatus};

#[derive(Clone)]
pub(super) struct PendingHostKey {
    pub(super) key: PublicKey,
}

#[derive(Clone)]
pub(super) struct SshDiagnostic {
    app: AppHandle,
    request_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct SshDiagnosticEvent {
    level: &'static str,
    message: String,
}

pub(super) fn ssh_diagnostic_event_name(request_id: &str) -> String {
    format!("ssh://connect-diagnostic/{request_id}")
}

fn emit_ssh_diagnostic(
    app: &AppHandle,
    request_id: Option<&str>,
    level: &'static str,
    message: impl Into<String>,
) {
    let Some(request_id) = request_id else {
        return;
    };

    let _ = app.emit(
        &ssh_diagnostic_event_name(request_id),
        SshDiagnosticEvent {
            level,
            message: message.into(),
        },
    );
}

impl SshDiagnostic {
    pub(super) fn new(app: &AppHandle, request_id: Option<String>) -> Self {
        Self {
            app: app.clone(),
            request_id,
        }
    }

    pub(super) fn info(&self, message: impl Into<String>) {
        emit_ssh_diagnostic(&self.app, self.request_id.as_deref(), "info", message);
    }

    pub(super) fn error(&self, message: impl Into<String>) {
        emit_ssh_diagnostic(&self.app, self.request_id.as_deref(), "error", message);
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
    if let Some(result) = verifier.last_result() {
        if result.status != HostKeyCheckStatus::Trusted {
            return host_key_error_message(&result);
        }
    }
    format!("SSH connection error: {}", error)
}

pub(super) fn emit_host_key_diagnostic(diagnostic: &SshDiagnostic, result: &HostKeyCheckResult) {
    diagnostic.info(format!(
        "target: host key received {} SHA256:{}",
        result.algorithm, result.fingerprint
    ));
    match result.status {
        HostKeyCheckStatus::Trusted => diagnostic.info("target: host key trusted"),
        HostKeyCheckStatus::Unknown => diagnostic.info("target: host key unknown"),
        HostKeyCheckStatus::Mismatch => diagnostic.info("target: host key mismatch"),
    }
}

pub(super) fn normalize_diagnostic_role(value: Option<String>) -> &'static str {
    match value.as_deref() {
        Some("jump") => "jump",
        _ => "target",
    }
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
