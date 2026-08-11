use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use russh::keys::PublicKey;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use tokio::time;
use uuid::Uuid;

use crate::ssh_known_hosts::{
    write_trusted_host_with_path, HostKeyCheckResult, HostKeyCheckStatus,
};

const HOST_KEY_PROMPT_EVENT: &str = "ssh://host-key-prompt";
const HOST_KEY_PROMPT_DISMISSED_EVENT: &str = "ssh://host-key-prompt-dismissed";
pub(super) const HOST_KEY_PROMPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(super) const HOST_KEY_PROMPT_CANCELLED: &str = "The SSH host key prompt was cancelled";
pub(super) const HOST_KEY_PROMPT_TIMEOUT_ERROR: &str = "The SSH host key prompt timed out";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshHostKeyPromptPayload {
    request_id: String,
    phase: String,
    #[serde(flatten)]
    result: HostKeyCheckResult,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshHostKeyPromptDismissedPayload {
    request_id: String,
}

struct PendingHostKeyPrompt {
    window_id: String,
    connect_request_id: Option<String>,
    sender: oneshot::Sender<bool>,
}

#[derive(Clone, Default)]
pub struct SshHostKeyPromptState {
    pending: Arc<Mutex<HashMap<String, PendingHostKeyPrompt>>>,
}

#[derive(Clone)]
pub(super) struct SshHostKeyPrompter {
    app: AppHandle,
    state: SshHostKeyPromptState,
    window_id: String,
    connect_request_id: Option<String>,
}

impl SshHostKeyPrompter {
    pub(super) fn new(
        app: &AppHandle,
        state: SshHostKeyPromptState,
        window_id: String,
        connect_request_id: Option<String>,
    ) -> Self {
        Self {
            app: app.clone(),
            state,
            window_id,
            connect_request_id,
        }
    }

    pub(super) async fn confirm(
        &self,
        phase: &'static str,
        result: HostKeyCheckResult,
        key: &PublicKey,
        known_hosts_path: &Path,
    ) -> Result<(), String> {
        let request_id = Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        self.state.pending.lock().await.insert(
            request_id.clone(),
            PendingHostKeyPrompt {
                window_id: self.window_id.clone(),
                connect_request_id: self.connect_request_id.clone(),
                sender,
            },
        );

        let payload = SshHostKeyPromptPayload {
            request_id: request_id.clone(),
            phase: phase.to_string(),
            result: result.clone(),
        };
        if let Err(error) = self
            .app
            .emit_to(&self.window_id, HOST_KEY_PROMPT_EVENT, &payload)
        {
            self.state.pending.lock().await.remove(&request_id);
            return Err(format!("Failed to show the SSH host key prompt: {error}"));
        }

        let accepted = self
            .state
            .wait_for_response(&request_id, receiver, HOST_KEY_PROMPT_TIMEOUT)
            .await;
        if matches!(&accepted, Err(error) if error == HOST_KEY_PROMPT_TIMEOUT_ERROR || error == HOST_KEY_PROMPT_CANCELLED)
        {
            let _ = self.app.emit_to(
                &self.window_id,
                HOST_KEY_PROMPT_DISMISSED_EVENT,
                SshHostKeyPromptDismissedPayload {
                    request_id: request_id.clone(),
                },
            );
        }
        accepted?;

        trust_prompted_host_key(&result, key, known_hosts_path)
    }
}

fn trust_prompted_host_key(
    result: &HostKeyCheckResult,
    key: &PublicKey,
    known_hosts_path: &Path,
) -> Result<(), String> {
    write_trusted_host_with_path(
        &result.host,
        result.port,
        key,
        result.status == HostKeyCheckStatus::Mismatch,
        known_hosts_path,
    )
}

impl SshHostKeyPromptState {
    pub async fn cancel_connect_attempt(&self, connect_request_id: &str) {
        let mut pending = self.pending.lock().await;
        let request_ids = pending
            .iter()
            .filter_map(|(request_id, prompt)| {
                (prompt.connect_request_id.as_deref() == Some(connect_request_id))
                    .then(|| request_id.clone())
            })
            .collect::<Vec<_>>();
        for request_id in request_ids {
            if let Some(prompt) = pending.remove(&request_id) {
                let _ = prompt.sender.send(false);
            }
        }
    }

    async fn wait_for_response(
        &self,
        request_id: &str,
        receiver: oneshot::Receiver<bool>,
        timeout: Duration,
    ) -> Result<(), String> {
        match time::timeout(timeout, receiver).await {
            Ok(Ok(true)) => Ok(()),
            Ok(Ok(false)) => Err(HOST_KEY_PROMPT_CANCELLED.to_string()),
            Ok(Err(_)) => Err("The SSH host key prompt did not complete".to_string()),
            Err(_) => {
                self.pending.lock().await.remove(request_id);
                Err(HOST_KEY_PROMPT_TIMEOUT_ERROR.to_string())
            }
        }
    }

    pub async fn submit(&self, request_id: String, accept: bool) -> Result<(), String> {
        let prompt = self
            .pending
            .lock()
            .await
            .remove(&request_id)
            .ok_or_else(|| "The SSH host key prompt request was not found".to_string())?;
        prompt
            .sender
            .send(accept)
            .map_err(|_| "The SSH host key prompt request has already finished".to_string())
    }

    pub async fn cancel_window(&self, window_id: &str) {
        let mut pending = self.pending.lock().await;
        let request_ids = pending
            .iter()
            .filter_map(|(request_id, prompt)| {
                (prompt.window_id == window_id).then(|| request_id.clone())
            })
            .collect::<Vec<_>>();
        for request_id in request_ids {
            if let Some(prompt) = pending.remove(&request_id) {
                let _ = prompt.sender.send(false);
            }
        }
    }

    #[cfg(test)]
    pub(super) async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::ssh_known_hosts::inspect_host_key_with_path;

    struct TestKnownHosts {
        directory: PathBuf,
        path: PathBuf,
    }

    impl TestKnownHosts {
        fn new() -> Self {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("test-known-hosts")
                .join(Uuid::new_v4().to_string());
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join("known_hosts");
            Self { directory, path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestKnownHosts {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn test_key(base64: &str) -> PublicKey {
        russh::keys::parse_public_key_base64(base64).unwrap()
    }

    fn check_result(status: HostKeyCheckStatus) -> HostKeyCheckResult {
        HostKeyCheckResult {
            status,
            host: "example.com".to_string(),
            port: 22,
            algorithm: "ssh-ed25519".to_string(),
            fingerprint: "fingerprint".to_string(),
            known_fingerprint: None,
        }
    }

    async fn insert_prompt(
        state: &SshHostKeyPromptState,
        request_id: &str,
        window_id: &str,
    ) -> oneshot::Receiver<bool> {
        let (sender, receiver) = oneshot::channel();
        state.pending.lock().await.insert(
            request_id.to_string(),
            PendingHostKeyPrompt {
                window_id: window_id.to_string(),
                connect_request_id: None,
                sender,
            },
        );
        receiver
    }

    #[tokio::test]
    async fn response_removes_pending_prompt() {
        let state = SshHostKeyPromptState::default();
        let receiver = insert_prompt(&state, "request", "main").await;

        state.submit("request".to_string(), true).await.unwrap();

        assert!(receiver.await.unwrap());
        assert_eq!(state.pending_count().await, 0);
    }

    #[tokio::test]
    async fn rejection_returns_cancelled_without_writing_known_hosts() {
        let state = SshHostKeyPromptState::default();
        let receiver = insert_prompt(&state, "request", "main").await;
        let known_hosts = TestKnownHosts::new();

        state.submit("request".to_string(), false).await.unwrap();
        let error = state
            .wait_for_response("request", receiver, Duration::from_secs(1))
            .await
            .unwrap_err();

        assert_eq!(error, HOST_KEY_PROMPT_CANCELLED);
        assert!(!known_hosts.path().exists());
    }

    #[tokio::test]
    async fn cancelling_a_window_only_clears_its_prompts() {
        let state = SshHostKeyPromptState::default();
        let first = insert_prompt(&state, "first", "main").await;
        let second = insert_prompt(&state, "second", "secondary").await;

        state.cancel_window("main").await;

        assert!(!first.await.unwrap());
        assert_eq!(state.pending_count().await, 1);
        state.submit("second".to_string(), true).await.unwrap();
        assert!(second.await.unwrap());
    }

    #[tokio::test]
    async fn cancelling_connect_attempt_only_clears_its_prompts() {
        let state = SshHostKeyPromptState::default();
        let (matching_sender, matching_receiver) = oneshot::channel();
        let (other_sender, _other_receiver) = oneshot::channel();
        let mut pending = state.pending.lock().await;
        pending.insert(
            "matching".to_string(),
            PendingHostKeyPrompt {
                window_id: "main".to_string(),
                connect_request_id: Some("connect-1".to_string()),
                sender: matching_sender,
            },
        );
        pending.insert(
            "other".to_string(),
            PendingHostKeyPrompt {
                window_id: "main".to_string(),
                connect_request_id: Some("connect-2".to_string()),
                sender: other_sender,
            },
        );
        drop(pending);

        state.cancel_connect_attempt("connect-1").await;

        assert!(!matching_receiver.await.unwrap());
        assert_eq!(state.pending_count().await, 1);
    }

    #[tokio::test]
    async fn timeout_clears_pending_prompt() {
        let state = SshHostKeyPromptState::default();
        let receiver = insert_prompt(&state, "request", "main").await;

        let error = state
            .wait_for_response("request", receiver, Duration::from_millis(1))
            .await
            .unwrap_err();

        assert_eq!(error, HOST_KEY_PROMPT_TIMEOUT_ERROR);
        assert_eq!(state.pending_count().await, 0);
    }

    #[test]
    fn accepting_unknown_key_writes_trusted_entry() {
        let known_hosts = TestKnownHosts::new();
        let key = test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ");

        trust_prompted_host_key(
            &check_result(HostKeyCheckStatus::Unknown),
            &key,
            known_hosts.path(),
        )
        .unwrap();

        assert_eq!(
            inspect_host_key_with_path("example.com", 22, &key, known_hosts.path())
                .unwrap()
                .status,
            HostKeyCheckStatus::Trusted
        );
    }

    #[test]
    fn accepting_mismatch_replaces_saved_entry() {
        let known_hosts = TestKnownHosts::new();
        let old_key =
            test_key("AAAAC3NzaC1lZDI1NTE5AAAAILIG2T/B0l0gaqj3puu510tu9N1OkQ4znY3LYuEm5zCF");
        let new_key =
            test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ");
        write_trusted_host_with_path("example.com", 22, &old_key, false, known_hosts.path())
            .unwrap();

        trust_prompted_host_key(
            &check_result(HostKeyCheckStatus::Mismatch),
            &new_key,
            known_hosts.path(),
        )
        .unwrap();

        assert_eq!(
            inspect_host_key_with_path("example.com", 22, &new_key, known_hosts.path())
                .unwrap()
                .status,
            HostKeyCheckStatus::Trusted
        );
    }
}
