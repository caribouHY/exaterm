use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use tokio::sync::{watch, Mutex, Notify};

pub(super) type SerialSessions = Arc<Mutex<HashMap<String, SerialSession>>>;

pub(super) struct SerialSession {
    pub shutdown: Arc<SerialShutdown>,
    pub writer: Option<mpsc::Sender<Vec<u8>>>,
}

pub(super) struct SerialShutdown {
    pub running: Arc<AtomicBool>,
    requested: Notify,
    completed: watch::Sender<Option<Result<(), String>>>,
}

impl SerialShutdown {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            running: Arc::new(AtomicBool::new(true)),
            requested: Notify::new(),
            completed: watch::channel(None).0,
        })
    }

    pub fn request(&self) {
        if self.running.swap(false, Ordering::SeqCst) {
            self.requested.notify_one();
        }
    }

    pub async fn wait(&self) -> Result<(), String> {
        self.completed
            .subscribe()
            .wait_for(|result| result.is_some())
            .await
            .expect("shutdown coordinator retains the completion sender")
            .as_ref()
            .expect("completion was observed")
            .clone()
    }
}

pub(super) struct RegistrationGuard(pub Option<Arc<SerialShutdown>>);

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        if let Some(shutdown) = &self.0 {
            // Cancelling the connect caller must not strand its unpublished workers.
            shutdown.request();
        }
    }
}

pub(super) async fn enqueue_data(
    sessions: &SerialSessions,
    session_id: &str,
    data: Vec<u8>,
) -> Result<(), String> {
    let sessions = sessions.lock().await;
    let session = sessions.get(session_id).ok_or("Session not found")?;
    let writer = session
        .writer
        .as_ref()
        .filter(|_| session.shutdown.running.load(Ordering::SeqCst))
        .ok_or("Session not found")?;
    writer
        .send(data)
        .map_err(|error| format!("Failed to send data: {error}"))
}

pub(super) async fn request_shutdown(
    sessions: &SerialSessions,
    session_id: &str,
) -> Option<Arc<SerialShutdown>> {
    let mut sessions = sessions.lock().await;
    let session = sessions.get_mut(session_id)?;
    session.shutdown.request();
    session.writer.take();
    Some(session.shutdown.clone())
}

pub(super) fn spawn_shutdown_coordinator<Cleanup, Finalize, FinalizeFuture>(
    sessions: SerialSessions,
    session_id: String,
    shutdown: Arc<SerialShutdown>,
    cleanup: Cleanup,
    finalize: Finalize,
) where
    Cleanup: Future<Output = Result<(), String>> + Send + 'static,
    Finalize: FnOnce() -> FinalizeFuture + Send + 'static,
    FinalizeFuture: Future<Output = Result<(), String>> + Send,
{
    tokio::spawn(async move {
        shutdown.requested.notified().await;
        let cleanup_result = cleanup.await;
        let finalize_result = finalize().await;
        let result = cleanup_result.and(finalize_result);
        let mut sessions = sessions.lock().await;
        // Publish completion and remove the entry under the same acceptance lock.
        shutdown.completed.send_replace(Some(result));
        sessions.remove(&session_id);
    });
}
