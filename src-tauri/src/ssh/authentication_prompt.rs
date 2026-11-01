use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::host_key::HostKeyVerifier;
use super::password_auth::PasswordAnswer;
use super::password_cache::{PasswordCache, PasswordCacheKey};
use crate::connect_attempt::ConnectAttempt;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use tokio::time;
use uuid::Uuid;
use zeroize::Zeroizing;

struct AuthenticationResponse {
    responses: Vec<String>,
    remember_password: bool,
}

impl std::fmt::Debug for AuthenticationResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthenticationResponse")
            .finish_non_exhaustive()
    }
}

const AUTHENTICATION_PROMPT_EVENT: &str = "ssh://authentication-prompt";
const AUTHENTICATION_PROMPT_DISMISSED_EVENT: &str = "ssh://authentication-prompt-dismissed";
const AUTHENTICATION_PROMPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(super) const AUTHENTICATION_PROMPT_CANCELLED: &str =
    "The SSH authentication prompt was cancelled";
pub(super) const AUTHENTICATION_PROMPT_TIMEOUT_ERROR: &str =
    "The SSH authentication prompt timed out";
pub(super) const AUTHENTICATION_PROMPT_RESPONSE_MISMATCH: &str =
    "The SSH authentication response count does not match the prompt count";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthenticationPrompt {
    pub prompt: String,
    pub echo: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthenticationPromptPayload {
    pub request_id: String,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub method: String,
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<SshAuthenticationPrompt>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshAuthenticationPromptDismissedPayload {
    request_id: String,
}

struct PendingAuthenticationPrompt {
    window_id: String,
    connect_request_id: Option<String>,
    expected_responses: usize,
    password_prompt: bool,
    sender: oneshot::Sender<Option<AuthenticationResponse>>,
}

#[derive(Clone, Default)]
pub struct SshAuthenticationPromptState {
    pending: Arc<Mutex<HashMap<String, PendingAuthenticationPrompt>>>,
}

#[derive(Clone)]
pub(super) struct SshAuthenticationPrompter {
    app: AppHandle,
    state: SshAuthenticationPromptState,
    window_id: String,
    connect_request_id: Option<String>,
    pub(super) cache: PasswordCache,
    pub(super) cache_generation: u64,
}

pub(super) struct SshAuthenticationContext<'a> {
    pub(super) prompter: &'a SshAuthenticationPrompter,
    pub(super) phase: &'static str,
    pub(super) host: &'a str,
    pub(super) port: u16,
    pub(super) username: &'a str,
    pub(super) profile_name: Option<&'a str>,
    pub(super) cache_key: Option<PasswordCacheKey>,
    pub(super) attempt: Option<&'a ConnectAttempt>,
}

impl SshAuthenticationPrompter {
    pub(super) fn new(
        app: &AppHandle,
        state: SshAuthenticationPromptState,
        window_id: String,
        connect_request_id: Option<String>,
        cache: PasswordCache,
    ) -> Self {
        Self {
            app: app.clone(),
            state,
            window_id,
            connect_request_id,
            cache_generation: cache.generation(),
            cache,
        }
    }

    pub(super) fn context<'a>(
        &'a self,
        phase: &'static str,
        host: &'a str,
        port: u16,
        username: &'a str,
        profile_name: Option<&'a str>,
    ) -> SshAuthenticationContext<'a> {
        SshAuthenticationContext {
            prompter: self,
            profile_name,
            phase,
            host,
            port,
            username,
            cache_key: None,
            attempt: None,
        }
    }

    async fn request(
        &self,
        mut payload: SshAuthenticationPromptPayload,
    ) -> Result<AuthenticationResponse, String> {
        if let Some(responses) = zero_prompt_responses(&payload.prompts) {
            return Ok(AuthenticationResponse {
                responses,
                remember_password: false,
            });
        }

        let request_id = Uuid::new_v4().to_string();
        let expected_responses = payload.prompts.len();
        let (sender, receiver) = oneshot::channel();
        self.state.pending.lock().await.insert(
            request_id.clone(),
            PendingAuthenticationPrompt {
                window_id: self.window_id.clone(),
                connect_request_id: self.connect_request_id.clone(),
                expected_responses,
                password_prompt: payload.method == "password",
                sender,
            },
        );
        payload.request_id = request_id.clone();

        if let Err(error) = self
            .app
            .emit_to(&self.window_id, AUTHENTICATION_PROMPT_EVENT, &payload)
        {
            self.state.pending.lock().await.remove(&request_id);
            return Err(format!(
                "Failed to show the SSH authentication prompt: {error}"
            ));
        }

        let result = self
            .state
            .wait_for_response(&request_id, receiver, AUTHENTICATION_PROMPT_TIMEOUT)
            .await;
        if matches!(&result, Err(error) if error == AUTHENTICATION_PROMPT_TIMEOUT_ERROR || error == AUTHENTICATION_PROMPT_CANCELLED)
        {
            let _ = self.app.emit_to(
                &self.window_id,
                AUTHENTICATION_PROMPT_DISMISSED_EVENT,
                SshAuthenticationPromptDismissedPayload {
                    request_id: request_id.clone(),
                },
            );
        }
        result
    }
}

fn zero_prompt_responses(prompts: &[SshAuthenticationPrompt]) -> Option<Vec<String>> {
    prompts.is_empty().then(Vec::new)
}

fn map_keyboard_interactive_prompts(
    prompts: Vec<russh::client::Prompt>,
) -> Vec<SshAuthenticationPrompt> {
    prompts
        .into_iter()
        .map(|prompt| SshAuthenticationPrompt {
            prompt: prompt.prompt,
            echo: prompt.echo,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn insert_prompt(
        state: &SshAuthenticationPromptState,
        request_id: &str,
        window_id: &str,
        expected_responses: usize,
    ) -> oneshot::Receiver<Option<AuthenticationResponse>> {
        let (sender, receiver) = oneshot::channel();
        state.pending.lock().await.insert(
            request_id.to_string(),
            PendingAuthenticationPrompt {
                window_id: window_id.to_string(),
                connect_request_id: None,
                expected_responses,
                password_prompt: true,
                sender,
            },
        );
        receiver
    }

    #[tokio::test]
    async fn remembers_only_password_prompts_and_rejects_late_responses() {
        let state = SshAuthenticationPromptState::default();
        let receiver = insert_prompt(&state, "password", "main", 1).await;
        state
            .submit_with_remember("password".into(), Some(vec!["secret".into()]), true)
            .await
            .unwrap();
        assert!(receiver.await.unwrap().unwrap().remember_password);
        assert!(state
            .submit_with_remember("password".into(), Some(vec!["late".into()]), true)
            .await
            .is_err());
        let receiver = insert_prompt(&state, "interactive", "main", 1).await;
        state
            .pending
            .lock()
            .await
            .get_mut("interactive")
            .unwrap()
            .password_prompt = false;
        assert!(state
            .submit_with_remember("interactive".into(), Some(vec!["otp".into()]), true)
            .await
            .is_err());
        state
            .submit("interactive".into(), Some(vec!["otp".into()]))
            .await
            .unwrap();
        assert!(!receiver.await.unwrap().unwrap().remember_password);
    }

    #[tokio::test]
    async fn submit_rejects_response_count_mismatch_without_removing_prompt() {
        let state = SshAuthenticationPromptState::default();
        let receiver = insert_prompt(&state, "request", "main", 2).await;

        let result = state
            .submit("request".to_string(), Some(vec!["only one".to_string()]))
            .await;

        assert_eq!(result.unwrap_err(), AUTHENTICATION_PROMPT_RESPONSE_MISMATCH);
        assert_eq!(state.pending_count().await, 1);

        state
            .submit(
                "request".to_string(),
                Some(vec!["first".to_string(), "second".to_string()]),
            )
            .await
            .unwrap();
        assert_eq!(
            receiver.await.unwrap().map(|answer| answer.responses),
            Some(vec!["first".to_string(), "second".to_string()])
        );
        assert_eq!(state.pending_count().await, 0);
    }

    #[tokio::test]
    async fn cancelling_a_window_only_clears_its_prompts() {
        let state = SshAuthenticationPromptState::default();
        let first = insert_prompt(&state, "first", "main", 1).await;
        let second = insert_prompt(&state, "second", "secondary", 1).await;

        state.cancel_window("main").await;

        assert_eq!(first.await.unwrap().map(|answer| answer.responses), None);
        assert_eq!(state.pending_count().await, 1);
        state
            .submit("second".to_string(), Some(vec!["answer".to_string()]))
            .await
            .unwrap();
        assert_eq!(
            second.await.unwrap().map(|answer| answer.responses),
            Some(vec!["answer".to_string()])
        );
    }

    #[tokio::test]
    async fn cancelling_connect_attempt_only_clears_its_prompts() {
        let state = SshAuthenticationPromptState::default();
        let (matching_sender, matching_receiver) = oneshot::channel();
        let (other_sender, _other_receiver) = oneshot::channel();
        let mut pending = state.pending.lock().await;
        pending.insert(
            "matching".to_string(),
            PendingAuthenticationPrompt {
                window_id: "main".to_string(),
                connect_request_id: Some("connect-1".to_string()),
                expected_responses: 1,
                password_prompt: true,
                sender: matching_sender,
            },
        );
        pending.insert(
            "other".to_string(),
            PendingAuthenticationPrompt {
                window_id: "main".to_string(),
                connect_request_id: Some("connect-2".to_string()),
                expected_responses: 1,
                password_prompt: true,
                sender: other_sender,
            },
        );
        drop(pending);

        state.cancel_connect_attempt("connect-1").await;

        assert_eq!(
            matching_receiver
                .await
                .unwrap()
                .map(|answer| answer.responses),
            None
        );
        assert_eq!(state.pending_count().await, 1);
    }

    #[tokio::test]
    async fn cancellation_response_clears_the_pending_prompt() {
        let state = SshAuthenticationPromptState::default();
        let receiver = insert_prompt(&state, "request", "main", 1).await;

        state.submit("request".to_string(), None).await.unwrap();

        assert_eq!(receiver.await.unwrap().map(|answer| answer.responses), None);
        assert_eq!(state.pending_count().await, 0);
    }

    #[tokio::test]
    async fn timeout_clears_the_pending_prompt() {
        let state = SshAuthenticationPromptState::default();
        let receiver = insert_prompt(&state, "request", "main", 1).await;

        let error = state
            .wait_for_response("request", receiver, Duration::from_millis(1))
            .await
            .unwrap_err();

        assert_eq!(error, AUTHENTICATION_PROMPT_TIMEOUT_ERROR);
        assert_eq!(state.pending_count().await, 0);
    }

    #[test]
    fn zero_question_requests_are_answered_automatically() {
        assert_eq!(zero_prompt_responses(&[]), Some(Vec::new()));
        assert_eq!(
            zero_prompt_responses(&[SshAuthenticationPrompt {
                prompt: "Password:".to_string(),
                echo: false,
            }]),
            None
        );
    }

    #[test]
    fn keyboard_interactive_prompts_preserve_text_order_and_echo_mode() {
        let prompts = map_keyboard_interactive_prompts(vec![
            russh::client::Prompt {
                prompt: "Password:".to_string(),
                echo: false,
            },
            russh::client::Prompt {
                prompt: "Account:".to_string(),
                echo: true,
            },
        ]);

        assert_eq!(
            prompts,
            vec![
                SshAuthenticationPrompt {
                    prompt: "Password:".to_string(),
                    echo: false,
                },
                SshAuthenticationPrompt {
                    prompt: "Account:".to_string(),
                    echo: true,
                },
            ]
        );
    }
}

impl<'a> SshAuthenticationContext<'a> {
    pub(super) fn with_verified_host(
        mut self,
        verifier: &HostKeyVerifier,
        attempt: Option<&'a ConnectAttempt>,
    ) -> Self {
        self.cache_key = verifier.last_result().map(|result| PasswordCacheKey {
            host: self.host.to_string(),
            port: self.port,
            username: self.username.to_string(),
            host_key: format!("{}:{}", result.algorithm, result.fingerprint),
        });
        self.attempt = attempt;
        self
    }

    pub(super) async fn request_password(&self) -> Result<PasswordAnswer, String> {
        let responses = self
            .request(
                "password",
                String::new(),
                String::new(),
                vec![SshAuthenticationPrompt {
                    prompt: String::new(),
                    echo: false,
                }],
            )
            .await?;
        responses
            .responses
            .into_iter()
            .next()
            .map(|password| PasswordAnswer {
                password: Zeroizing::new(password),
                remember: responses.remember_password,
            })
            .ok_or_else(|| {
                "The SSH password authentication prompt did not return a response".to_string()
            })
    }

    pub(super) async fn request_keyboard_interactive(
        &self,
        name: String,
        instructions: String,
        prompts: Vec<russh::client::Prompt>,
    ) -> Result<Vec<String>, String> {
        let prompts = map_keyboard_interactive_prompts(prompts);
        self.request("keyboard_interactive", name, instructions, prompts)
            .await
            .map(|answer| answer.responses)
    }

    async fn request(
        &self,
        method: &str,
        name: String,
        instructions: String,
        prompts: Vec<SshAuthenticationPrompt>,
    ) -> Result<AuthenticationResponse, String> {
        self.prompter
            .request(SshAuthenticationPromptPayload {
                request_id: String::new(),
                phase: self.phase.to_string(),
                profile_name: self.profile_name.map(ToOwned::to_owned),
                host: self.host.to_string(),
                port: self.port,
                username: self.username.to_string(),
                method: method.to_string(),
                name,
                instructions,
                prompts,
            })
            .await
    }
}

impl SshAuthenticationPromptState {
    pub async fn cancel_connect_attempt(&self, connect_request_id: &str) {
        let mut pending = self.pending.lock().await;
        let request_ids = pending
            .iter()
            .filter(|(_, prompt)| prompt.connect_request_id.as_deref() == Some(connect_request_id))
            .map(|(request_id, _)| request_id.clone())
            .collect::<Vec<_>>();
        for request_id in request_ids {
            if let Some(prompt) = pending.remove(&request_id) {
                let _ = prompt.sender.send(None);
            }
        }
    }

    async fn wait_for_response(
        &self,
        request_id: &str,
        receiver: oneshot::Receiver<Option<AuthenticationResponse>>,
        timeout: Duration,
    ) -> Result<AuthenticationResponse, String> {
        match time::timeout(timeout, receiver).await {
            Ok(Ok(Some(responses))) => Ok(responses),
            Ok(Ok(None)) => Err(AUTHENTICATION_PROMPT_CANCELLED.to_string()),
            Ok(Err(_)) => Err("The SSH authentication prompt did not complete".to_string()),
            Err(_) => {
                self.pending.lock().await.remove(request_id);
                Err(AUTHENTICATION_PROMPT_TIMEOUT_ERROR.to_string())
            }
        }
    }

    #[cfg(test)]
    pub async fn submit(
        &self,
        request_id: String,
        responses: Option<Vec<String>>,
    ) -> Result<(), String> {
        self.submit_with_remember(request_id, responses, false)
            .await
    }

    pub async fn submit_with_remember(
        &self,
        request_id: String,
        responses: Option<Vec<String>>,
        remember_password: bool,
    ) -> Result<(), String> {
        let mut pending = self.pending.lock().await;
        let prompt = pending
            .get(&request_id)
            .ok_or_else(|| "The SSH authentication prompt request was not found".to_string())?;
        if remember_password && !prompt.password_prompt {
            return Err("Only SSH password authentication can be remembered".to_string());
        }
        if let Some(responses) = &responses {
            if responses.len() != prompt.expected_responses {
                return Err(AUTHENTICATION_PROMPT_RESPONSE_MISMATCH.to_string());
            }
        }
        let prompt = pending
            .remove(&request_id)
            .expect("the pending SSH authentication prompt was checked above");
        prompt
            .sender
            .send(responses.map(|responses| AuthenticationResponse {
                responses,
                remember_password,
            }))
            .map_err(|_| "The SSH authentication prompt request has already finished".to_string())
    }

    pub async fn cancel_window(&self, window_id: &str) {
        let mut pending = self.pending.lock().await;
        let request_ids = pending
            .iter()
            .filter(|(_, prompt)| prompt.window_id == window_id)
            .map(|(request_id, _)| request_id.clone())
            .collect::<Vec<_>>();
        for request_id in request_ids {
            if let Some(prompt) = pending.remove(&request_id) {
                let _ = prompt.sender.send(None);
            }
        }
    }

    #[cfg(test)]
    pub(super) async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }
}
