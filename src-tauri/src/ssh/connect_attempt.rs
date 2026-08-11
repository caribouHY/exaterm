use std::collections::{hash_map::Entry, HashMap};
use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::watch;

pub(super) const SSH_CONNECT_CANCELLED: &str = "The SSH connection attempt was cancelled";

#[derive(Clone, Default)]
pub struct SshConnectAttemptState {
    pending: Arc<StdMutex<HashMap<String, watch::Sender<bool>>>>,
}

pub(crate) struct SshConnectAttempt {
    request_id: String,
    state: SshConnectAttemptState,
    cancellation: watch::Receiver<bool>,
    registered: bool,
}

pub(super) async fn run_with_attempt<F, T>(
    attempt: Option<&SshConnectAttempt>,
    operation: F,
) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    match attempt {
        Some(attempt) => attempt.run(operation).await,
        None => operation.await,
    }
}

impl SshConnectAttemptState {
    pub(super) fn register(&self, request_id: String) -> Result<SshConnectAttempt, String> {
        let (sender, cancellation) = watch::channel(false);
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match pending.entry(request_id.clone()) {
            Entry::Occupied(_) => {
                return Err(
                    "An SSH connection attempt with this request ID already exists".to_string(),
                );
            }
            Entry::Vacant(entry) => {
                entry.insert(sender);
            }
        }
        Ok(SshConnectAttempt {
            request_id,
            state: self.clone(),
            cancellation,
            registered: true,
        })
    }

    pub(super) fn cancel(&self, request_id: &str) -> bool {
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(sender) = pending.get(request_id) else {
            return false;
        };
        sender.send_replace(true);
        true
    }

    fn remove(&self, request_id: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(request_id);
    }
}

impl SshConnectAttempt {
    pub(super) async fn run<F, T>(&self, operation: F) -> Result<T, String>
    where
        F: Future<Output = Result<T, String>>,
    {
        let mut cancellation = self.cancellation.clone();
        if *cancellation.borrow() {
            return Err(SSH_CONNECT_CANCELLED.to_string());
        }
        tokio::select! {
            biased;
            changed = cancellation.wait_for(|cancelled| *cancelled) => {
                let _ = changed;
                Err(SSH_CONNECT_CANCELLED.to_string())
            }
            result = operation => result,
        }
    }

    pub(super) fn begin_completion(&mut self) -> bool {
        let mut pending = self
            .state
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cancelled = pending
            .get(&self.request_id)
            .is_none_or(|sender| *sender.borrow());
        pending.remove(&self.request_id);
        self.registered = false;
        !cancelled
    }
}

impl Drop for SshConnectAttempt {
    fn drop(&mut self) {
        if self.registered {
            self.state.remove(&self.request_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelling_registered_attempt_interrupts_pending_operation() {
        let state = SshConnectAttemptState::default();
        let attempt = state.register("request-1".to_string()).unwrap();

        assert!(state.cancel("request-1"));
        let error = attempt
            .run(std::future::pending::<Result<(), String>>())
            .await
            .unwrap_err();

        assert_eq!(error, SSH_CONNECT_CANCELLED);
    }

    #[test]
    fn completion_wins_against_late_cancellation() {
        let state = SshConnectAttemptState::default();
        let mut attempt = state.register("request-1".to_string()).unwrap();

        assert!(attempt.begin_completion());
        assert!(!state.cancel("request-1"));
    }

    #[test]
    fn cancellation_wins_before_completion_boundary() {
        let state = SshConnectAttemptState::default();
        let mut attempt = state.register("request-1".to_string()).unwrap();

        assert!(state.cancel("request-1"));
        assert!(!attempt.begin_completion());
    }

    #[test]
    fn dropping_attempt_unregisters_it() {
        let state = SshConnectAttemptState::default();
        let attempt = state.register("request-1".to_string()).unwrap();
        drop(attempt);

        assert!(!state.cancel("request-1"));
    }
}
