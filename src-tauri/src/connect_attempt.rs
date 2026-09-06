use std::collections::{hash_map::Entry, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::watch;

#[derive(Clone)]
pub struct ConnectAttemptState {
    pending: Arc<StdMutex<HashMap<String, watch::Sender<bool>>>>,
    cancelled_message: &'static str,
    duplicate_message: &'static str,
}

pub struct ConnectAttempt {
    request_id: String,
    state: ConnectAttemptState,
    cancellation: watch::Receiver<bool>,
    registered: bool,
}

pub async fn run_with_attempt<F, T>(
    attempt: Option<&ConnectAttempt>,
    operation: Pin<Box<F>>,
) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    match attempt {
        Some(attempt) => attempt.run(operation).await,
        None => operation.await,
    }
}

impl ConnectAttemptState {
    pub fn new(cancelled_message: &'static str, duplicate_message: &'static str) -> Self {
        Self {
            pending: Arc::new(StdMutex::new(HashMap::new())),
            cancelled_message,
            duplicate_message,
        }
    }

    pub fn register(&self, request_id: String) -> Result<ConnectAttempt, String> {
        let (sender, cancellation) = watch::channel(false);
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match pending.entry(request_id.clone()) {
            Entry::Occupied(_) => return Err(self.duplicate_message.to_string()),
            Entry::Vacant(entry) => {
                entry.insert(sender);
            }
        }
        Ok(ConnectAttempt {
            request_id,
            state: self.clone(),
            cancellation,
            registered: true,
        })
    }

    pub fn cancel(&self, request_id: &str) -> bool {
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

impl ConnectAttempt {
    pub async fn run<F, T>(&self, operation: Pin<Box<F>>) -> Result<T, String>
    where
        F: Future<Output = Result<T, String>>,
    {
        let mut cancellation = self.cancellation.clone();
        if *cancellation.borrow() {
            return Err(self.state.cancelled_message.to_string());
        }
        tokio::select! {
            biased;
            changed = cancellation.wait_for(|cancelled| *cancelled) => {
                let _ = changed;
                Err(self.state.cancelled_message.to_string())
            }
            result = operation => result,
        }
    }

    pub fn begin_completion(&mut self) -> bool {
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

impl Drop for ConnectAttempt {
    fn drop(&mut self) {
        if self.registered {
            self.state.remove(&self.request_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    const CANCELLED: &str = "The connection attempt was cancelled";
    const DUPLICATE: &str = "A connection attempt with this request ID already exists";

    fn state() -> ConnectAttemptState {
        ConnectAttemptState::new(CANCELLED, DUPLICATE)
    }

    #[derive(Debug)]
    struct DropProbe(Arc<AtomicBool>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn cancelling_registered_attempt_interrupts_pending_operation() {
        let state = state();
        let attempt = state.register("request-1".to_string()).unwrap();

        assert!(state.cancel("request-1"));
        let error = attempt
            .run(Box::pin(std::future::pending::<Result<(), String>>()))
            .await
            .unwrap_err();

        assert_eq!(error, CANCELLED);
    }

    #[tokio::test]
    async fn cancellation_drops_a_late_blocking_result() {
        let state = state();
        let attempt = state.register("request-1".to_string()).unwrap();
        let dropped = Arc::new(AtomicBool::new(false));
        let task_dropped = dropped.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let operation = async move {
            tokio::task::spawn_blocking(move || {
                let _ = started_tx.send(());
                let _ = release_rx.recv();
                Ok(DropProbe(task_dropped))
            })
            .await
            .map_err(|error| error.to_string())?
        };
        let run_task = tokio::spawn(async move { attempt.run(Box::pin(operation)).await });

        started_rx.await.unwrap();
        assert!(state.cancel("request-1"));
        assert_eq!(run_task.await.unwrap().unwrap_err(), CANCELLED);
        release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !dropped.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("late blocking result should be dropped");
    }

    #[test]
    fn rejects_duplicate_request_ids() {
        let state = state();
        let _attempt = state.register("request-1".to_string()).unwrap();

        assert_eq!(
            state.register("request-1".to_string()).err().unwrap(),
            DUPLICATE
        );
    }

    #[test]
    fn completion_wins_against_late_cancellation() {
        let state = state();
        let mut attempt = state.register("request-1".to_string()).unwrap();

        assert!(attempt.begin_completion());
        assert!(!state.cancel("request-1"));
    }

    #[test]
    fn cancellation_wins_before_completion_boundary() {
        let state = state();
        let mut attempt = state.register("request-1".to_string()).unwrap();

        assert!(state.cancel("request-1"));
        assert!(!attempt.begin_completion());
    }

    #[test]
    fn dropping_attempt_unregisters_it() {
        let state = state();
        let attempt = state.register("request-1".to_string()).unwrap();
        drop(attempt);

        assert!(!state.cancel("request-1"));
    }
}
