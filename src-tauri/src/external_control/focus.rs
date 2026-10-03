use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use super::service::ExternalControlFocusRequestPayload;

#[derive(Clone, Default)]
pub struct ExternalControlFocusState {
    pending: Arc<Mutex<HashMap<String, PendingFocus>>>,
}

struct PendingFocus {
    window_id: String,
    tab_id: String,
    sender: oneshot::Sender<bool>,
}

struct PendingFocusGuard {
    state: ExternalControlFocusState,
    request_id: String,
}

impl Drop for PendingFocusGuard {
    fn drop(&mut self) {
        self.state.pending.lock().unwrap().remove(&self.request_id);
    }
}

impl ExternalControlFocusState {
    pub(crate) async fn request(
        &self,
        app: &AppHandle,
        payload: ExternalControlFocusRequestPayload,
    ) -> Result<bool, String> {
        self.request_with_sender(payload, |payload| {
            app.emit_to(
                &payload.snapshot.window_id,
                "external-control://session-focus-request",
                payload,
            )
            .map_err(|_| "Failed to send the session focus request".to_string())
        })
        .await
    }

    pub(crate) async fn request_with_sender<F>(
        &self,
        payload: ExternalControlFocusRequestPayload,
        send: F,
    ) -> Result<bool, String>
    where
        F: FnOnce(&ExternalControlFocusRequestPayload) -> Result<(), String>,
    {
        let (sender, receiver) = oneshot::channel();
        let _guard = PendingFocusGuard {
            state: self.clone(),
            request_id: payload.request_id.clone(),
        };
        self.pending.lock().unwrap().insert(
            payload.request_id.clone(),
            PendingFocus {
                window_id: payload.snapshot.window_id.clone(),
                tab_id: payload.tab_id.clone(),
                sender,
            },
        );
        send(&payload)?;
        receiver
            .await
            .map_err(|_| "The session focus request did not complete".to_string())
    }

    pub(crate) fn submit(
        &self,
        window_id: &str,
        request_id: &str,
        tab_id: &str,
        selected: bool,
    ) -> Result<(), String> {
        let mut pending = self.pending.lock().unwrap();
        let request = pending
            .get(request_id)
            .ok_or("Session focus request not found")?;
        if request.window_id != window_id || request.tab_id != tab_id {
            return Err("Session focus response does not match the request".into());
        }
        let request = pending
            .remove(request_id)
            .expect("request checked under the same lock");
        request
            .sender
            .send(selected)
            .map_err(|_| "Session focus request has already finished".into())
    }
}

#[tauri::command]
pub fn external_control_session_focus_submit(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, ExternalControlFocusState>,
    request_id: String,
    tab_id: String,
    selected: bool,
) -> Result<(), String> {
    state.submit(window.label(), &request_id, &tab_id, selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WorkspaceState;
    use std::time::Duration;

    async fn payload() -> ExternalControlFocusRequestPayload {
        ExternalControlFocusRequestPayload {
            request_id: "request".into(),
            session_id: "session".into(),
            tab_id: "tab".into(),
            snapshot: WorkspaceState::new()
                .register_window("main".into(), "main".into(), false)
                .await,
        }
    }

    #[tokio::test]
    async fn responses_require_the_expected_window_and_tab() {
        let state = ExternalControlFocusState::default();
        let response_state = state.clone();
        assert!(state
            .request_with_sender(payload().await, move |payload| {
                assert!(response_state
                    .submit("other", &payload.request_id, &payload.tab_id, true)
                    .is_err());
                assert!(response_state
                    .submit("main", &payload.request_id, "other-tab", true)
                    .is_err());
                response_state.submit("main", &payload.request_id, &payload.tab_id, true)
            })
            .await
            .unwrap());
        assert!(state.pending.lock().unwrap().is_empty());
        assert!(state.submit("main", "request", "tab", true).is_err());
    }

    #[tokio::test]
    async fn failed_delivery_and_timed_out_requests_release_pending_entries() {
        let state = ExternalControlFocusState::default();
        assert!(state
            .request_with_sender(payload().await, |_| Err("delivery failed".into()))
            .await
            .is_err());
        assert!(state.pending.lock().unwrap().is_empty());
        assert!(tokio::time::timeout(
            Duration::from_millis(10),
            state.request_with_sender(payload().await, |_| Ok(()))
        )
        .await
        .is_err());
        assert!(state.pending.lock().unwrap().is_empty());
        assert!(state.submit("main", "request", "tab", true).is_err());
    }
}
