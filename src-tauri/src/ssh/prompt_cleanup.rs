use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

pub(super) struct PromptCleanup<T: Send + 'static, F: FnOnce(String) + Send + 'static> {
    pending: Arc<Mutex<HashMap<String, T>>>,
    dismissal: Option<(String, F)>,
}

impl<T: Send + 'static, F: FnOnce(String) + Send + 'static> PromptCleanup<T, F> {
    pub(super) fn new(
        pending: Arc<Mutex<HashMap<String, T>>>,
        request_id: String,
        dismiss: F,
    ) -> Self {
        Self {
            pending,
            dismissal: Some((request_id, dismiss)),
        }
    }

    pub(super) async fn finish(&mut self) {
        let mut pending = self.pending.lock().await;
        if let Some((request_id, dismiss)) = self.dismissal.take() {
            pending.remove(&request_id);
            drop(pending);
            dismiss(request_id);
        }
    }
}

impl<T: Send + 'static, F: FnOnce(String) + Send + 'static> Drop for PromptCleanup<T, F> {
    fn drop(&mut self) {
        if let Some((request_id, dismiss)) = self.dismissal.take() {
            let pending = self.pending.clone();
            // Connection cancellation drops the authentication future before its normal cleanup.
            tauri::async_runtime::spawn(async move {
                pending.lock().await.remove(&request_id);
                dismiss(request_id);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn finishing_and_dropping_dismiss_only_the_owned_request() {
        for finish in [false, true] {
            let pending = Arc::new(Mutex::new(HashMap::from([
                ("owned".to_string(), ()),
                ("other".to_string(), ()),
            ])));
            let (sender, receiver) = oneshot::channel();
            let mut cleanup = PromptCleanup::new(pending.clone(), "owned".into(), move |id| {
                sender.send(id).unwrap();
            });
            if finish {
                cleanup.finish().await;
            }
            drop(cleanup);
            assert_eq!(receiver.await.unwrap(), "owned");
            assert_eq!(*pending.lock().await, HashMap::from([("other".into(), ())]));
        }
    }

    #[tokio::test]
    async fn cancellation_while_finishing_still_dismisses_the_request() {
        let pending = Arc::new(Mutex::new(HashMap::from([("owned".to_string(), ())])));
        let (sender, receiver) = oneshot::channel();
        let mut cleanup = PromptCleanup::new(pending.clone(), "owned".into(), move |id| {
            sender.send(id).unwrap();
        });
        let locked = pending.lock().await;
        let mut finishing = Box::pin(cleanup.finish());
        assert!(matches!(
            futures::poll!(finishing.as_mut()),
            std::task::Poll::Pending
        ));
        drop(finishing);
        drop(cleanup);
        drop(locked);
        assert_eq!(receiver.await.unwrap(), "owned");
        assert!(pending.lock().await.is_empty());
    }
}
