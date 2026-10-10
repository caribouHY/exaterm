use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;

use crate::logger::{manual_log_session, start_log_on_connection, LoggerState};
use crate::terminal_control::TerminalStatus;

async fn fixture(
    state: &SerialState,
    terminals: &TerminalControlState,
    logger: Option<LoggerState>,
    cleanup: impl std::future::Future<Output = Result<(), String>> + Send + 'static,
    finalize_gate: Option<tokio::sync::oneshot::Receiver<()>>,
) -> (
    String,
    Arc<SerialShutdown>,
    mpsc::Receiver<Vec<u8>>,
    Arc<AtomicUsize>,
) {
    let sid = Uuid::new_v4().to_string();
    let shutdown = SerialShutdown::new();
    let (writer, rx) = mpsc::channel();
    terminals
        .register_session(sid.clone(), TerminalProtocol::Serial, "COM1".into())
        .await;
    state.sessions.lock().await.insert(
        sid.clone(),
        SerialSession {
            shutdown: shutdown.clone(),
            writer: Some(writer),
        },
    );
    let finalized = Arc::new(AtomicUsize::new(0));
    let finalize_count = finalized.clone();
    let finalize_terminals = terminals.clone();
    let finalize_sid = sid.clone();
    spawn_shutdown_coordinator(
        state.sessions.clone(),
        sid.clone(),
        shutdown.clone(),
        cleanup,
        move || async move {
            if let Some(gate) = finalize_gate {
                gate.await.unwrap();
            }
            finalize_terminals.mark_disconnected(&finalize_sid).await;
            if let Some(logger) = logger {
                logger::clear_session_logs(&logger, &finalize_sid).await;
            }
            finalize_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    );
    (sid, shutdown, rx, finalized)
}

#[tokio::test]
async fn parallel_disconnects_wait_for_release_and_finalization() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (finalize, finalize_rx) = tokio::sync::oneshot::channel();
    let released = Arc::new(AtomicBool::new(false));
    let port_released = released.clone();
    let (sid, shutdown, rx, finalized) = fixture(
        &state,
        &terminals,
        None,
        async move {
            entered.send(()).unwrap();
            release_rx.await.unwrap();
            port_released.store(true, Ordering::SeqCst);
            Ok(())
        },
        Some(finalize_rx),
    )
    .await;
    write_data(&state, &terminals, &sid, "before".into())
        .await
        .unwrap();
    let first = tokio::spawn({
        let sessions = state.sessions.clone();
        let sid = sid.clone();
        async move { shutdown_session(&sessions, &sid).await }
    });
    entered_rx.await.unwrap();
    assert!(!shutdown.running.load(Ordering::SeqCst));
    assert!(write_data(&state, &terminals, &sid, "after".into())
        .await
        .is_err());
    assert_eq!(rx.recv().unwrap(), b"before");
    assert!(rx.recv().is_err());
    let second = shutdown_session(&state.sessions, &sid);
    tokio::pin!(second);
    assert!(futures::poll!(&mut second).is_pending());
    assert!(!first.is_finished());
    assert!(!released.load(Ordering::SeqCst));
    assert_eq!(
        terminals.session_info(&sid).await.unwrap().status,
        TerminalStatus::Connected
    );
    release.send(()).unwrap();
    assert!(futures::poll!(&mut second).is_pending());
    finalize.send(()).unwrap();
    second.await.unwrap();
    first.await.unwrap().unwrap();
    assert!(released.load(Ordering::SeqCst));
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
    assert!(!state.sessions.lock().await.contains_key(&sid));
    assert_eq!(
        terminals.session_info(&sid).await.unwrap().status,
        TerminalStatus::Disconnected
    );
    shutdown_session(&state.sessions, &sid).await.unwrap();
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn aborting_first_disconnect_does_not_cancel_cleanup() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (sid, _, _, finalized) = fixture(
        &state,
        &terminals,
        None,
        async move {
            entered.send(()).unwrap();
            release_rx.await.unwrap();
            Ok(())
        },
        None,
    )
    .await;
    let first = tokio::spawn({
        let sessions = state.sessions.clone();
        let sid = sid.clone();
        async move { shutdown_session(&sessions, &sid).await }
    });
    entered_rx.await.unwrap();
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    let next = shutdown_session(&state.sessions, &sid);
    tokio::pin!(next);
    assert!(futures::poll!(&mut next).is_pending());
    release.send(()).unwrap();
    next.await.unwrap();
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn io_error_and_explicit_disconnect_share_one_completion() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let (sid, _, _, finalized) = fixture(
        &state,
        &terminals,
        None,
        async move {
            release_rx.await.unwrap();
            Ok(())
        },
        None,
    )
    .await;
    let error_shutdown = request_shutdown(&state.sessions, &sid).await.unwrap();
    let explicit = shutdown_session(&state.sessions, &sid);
    tokio::pin!(explicit);
    assert!(futures::poll!(&mut explicit).is_pending());
    let concurrent_error = request_shutdown(&state.sessions, &sid).await.unwrap();
    assert!(Arc::ptr_eq(&error_shutdown, &concurrent_error));
    release.send(()).unwrap();
    explicit.await.unwrap();
    error_shutdown.wait().await.unwrap();
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn encoded_input_cannot_enqueue_after_disconnect_cutoff() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let (sid, _, rx, _) = fixture(
        &state,
        &terminals,
        None,
        async move {
            release_rx.await.unwrap();
            Ok(())
        },
        None,
    )
    .await;
    // Holding the production acceptance lock pauses write_data after its encoding await.
    let lock = state.sessions.lock().await;
    let write = write_data(&state, &terminals, &sid, "late".into());
    tokio::pin!(write);
    assert!(futures::poll!(&mut write).is_pending());
    lock.get(&sid).unwrap().shutdown.request();
    drop(lock);
    assert_eq!(write.await.unwrap_err(), "Session not found");
    request_shutdown(&state.sessions, &sid).await.unwrap();
    assert!(rx.try_recv().is_err());
    release.send(()).unwrap();
    shutdown_session(&state.sessions, &sid).await.unwrap();
}

#[tokio::test]
async fn cleanup_failure_is_shared_and_still_finalizes() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let (sid, shutdown, _, finalized) = fixture(
        &state,
        &terminals,
        None,
        async { Err("purge failed".into()) },
        None,
    )
    .await;
    assert_eq!(
        shutdown_session(&state.sessions, &sid).await.unwrap_err(),
        "purge failed"
    );
    assert_eq!(shutdown.wait().await.unwrap_err(), "purge failed");
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn read_error_waits_for_pending_output_before_disconnect() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let (finish_output, output_finished) = tokio::sync::oneshot::channel();
    let (sid, shutdown, _, finalized) = fixture(
        &state,
        &terminals,
        None,
        async move {
            output_finished.await.unwrap();
            Ok(())
        },
        None,
    )
    .await;
    let events = Mutex::new(Vec::new());
    let entered = tokio::sync::Notify::new();
    let resume = tokio::sync::Notify::new();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(SerialReadEvent::Data(b"first".to_vec())).unwrap();
    tx.send(SerialReadEvent::Data(b"last".to_vec())).unwrap();
    tx.send(SerialReadEvent::Error("read failed".into()))
        .unwrap();
    drop(tx);
    let processing = process_serial_output(
        rx,
        |data| {
            let terminals = &terminals;
            let sid = &sid;
            let events = &events;
            let entered = &entered;
            let resume = &resume;
            async move {
                if data == b"first" {
                    entered.notify_one();
                    resume.notified().await;
                }
                terminals.append_output(sid, &data).await;
                events.lock().await.push(String::from_utf8(data).unwrap());
            }
        },
        |error| async {
            events.lock().await.push(error);
            request_shutdown(&state.sessions, &sid).await;
        },
    );
    tokio::pin!(processing);
    tokio::select! { _ = &mut processing => panic!("output must remain blocked"), _ = entered.notified() => {} }
    assert!(events.lock().await.is_empty());
    assert!(shutdown.running.load(Ordering::SeqCst));
    resume.notify_one();
    processing.await;
    assert_eq!(*events.lock().await, ["first", "last", "read failed"]);
    assert_eq!(finalized.load(Ordering::SeqCst), 0);
    finish_output.send(()).unwrap();
    shutdown.wait().await.unwrap();
    assert_eq!(
        terminals.read_output(&sid, 100).await.unwrap().output,
        "firstlast"
    );
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn closing_output_channel_drains_data_without_reporting_read_error() {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(SerialReadEvent::Data(b"last".to_vec())).unwrap();
    drop(tx);
    let output = Mutex::new(Vec::new());
    process_serial_output(
        rx,
        |data| async { output.lock().await.extend(data) },
        |_| async { panic!("channel closure is not a read error") },
    )
    .await;
    assert_eq!(*output.lock().await, b"last");
}

#[tokio::test]
async fn disconnect_keeps_logger_until_workers_finish() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("serial-log-test-{}", Uuid::new_v4()));
    let logger = LoggerState::with_paths(dir.clone(), dir.join("index.json"));
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let (sid, shutdown, _, _) = fixture(
        &state,
        &terminals,
        Some(logger.clone()),
        async move {
            release_rx.await.unwrap();
            Ok(())
        },
        None,
    )
    .await;
    start_log_on_connection(&logger, sid.clone(), "serial".into(), "COM1".into())
        .await
        .unwrap();
    request_shutdown(&state.sessions, &sid).await;
    assert!(manual_log_session(&logger, &sid).await.is_some());
    release.send(()).unwrap();
    shutdown.wait().await.unwrap();
    assert!(manual_log_session(&logger, &sid).await.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn abandoned_registration_still_runs_cleanup() {
    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let (sid, shutdown, _, finalized) =
        fixture(&state, &terminals, None, async { Ok(()) }, None).await;
    drop(RegistrationGuard(Some(shutdown.clone())));
    shutdown.wait().await.unwrap();
    assert!(!state.sessions.lock().await.contains_key(&sid));
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn production_cleanup_releases_every_handle_and_drains_output_before_success() {
    struct HandleLease(Arc<AtomicUsize>);
    impl HandleLease {
        fn new(handles: &Arc<AtomicUsize>) -> Self {
            handles.fetch_add(1, Ordering::SeqCst);
            Self(handles.clone())
        }
    }
    impl Drop for HandleLease {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    struct BlockingPort {
        _lease: HandleLease,
        entered: Option<tokio::sync::oneshot::Sender<()>>,
        cancel: mpsc::Receiver<()>,
    }
    impl std::io::Write for BlockingPort {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.entered.take().unwrap().send(()).unwrap();
            self.cancel.recv().unwrap();
            Err(std::io::ErrorKind::Interrupted.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            panic!("must discard rather than flush")
        }
    }
    for purge_fails in [false, true] {
        let state = SerialState::new();
        let terminals = TerminalControlState::new();
        let handles = Arc::new(AtomicUsize::new(0));
        let shutdown = SerialShutdown::new();
        let sid = Uuid::new_v4().to_string();
        let (writer, writes) = mpsc::channel();
        let (entered, entered_rx) = tokio::sync::oneshot::channel();
        let (cancel, cancellation) = mpsc::channel();
        let writer_thread = spawn_serial_writer(
            BlockingPort {
                _lease: HandleLease::new(&handles),
                entered: Some(entered),
                cancel: cancellation,
            },
            writes,
            shutdown.running.clone(),
            |_| panic!("cancellation should not report a write error"),
        )
        .unwrap();
        let reader_handle = HandleLease::new(&handles);
        let (finish_reader, finish_reader_rx) = tokio::sync::oneshot::channel();
        let reader_worker = tokio::spawn(async move {
            finish_reader_rx.await.unwrap();
            drop(reader_handle);
        });
        let control_handle = HandleLease::new(&handles);
        let (cleared, cleared_rx) = tokio::sync::oneshot::channel();
        let (drain_output, drain_output_rx) = tokio::sync::oneshot::channel();
        let retained = terminals.clone();
        let output_sid = sid.clone();
        let output_worker = tokio::spawn(async move {
            drain_output_rx.await.unwrap();
            retained.append_output(&output_sid, b"last received").await;
        });
        let cleanup = finish_serial_workers(
            writer_thread,
            move |_| {
                let _ = cancel.send(());
                Ok(())
            },
            move || {
                drop(control_handle);
                cleared.send(()).unwrap();
                if purge_fails {
                    Err("purge failed".into())
                } else {
                    Ok(())
                }
            },
            reader_worker,
            output_worker,
        );
        terminals
            .register_session(sid.clone(), TerminalProtocol::Serial, "COM1".into())
            .await;
        state.sessions.lock().await.insert(
            sid.clone(),
            SerialSession {
                shutdown: shutdown.clone(),
                writer: Some(writer),
            },
        );
        let finalized = Arc::new(AtomicUsize::new(0));
        let count = finalized.clone();
        let final_terminals = terminals.clone();
        let final_sid = sid.clone();
        let final_handles = handles.clone();
        spawn_shutdown_coordinator(
            state.sessions.clone(),
            sid.clone(),
            shutdown.clone(),
            cleanup,
            move || async move {
                assert_eq!(final_handles.load(Ordering::SeqCst), 0);
                assert_eq!(
                    final_terminals
                        .read_output(&final_sid, 100)
                        .await
                        .unwrap()
                        .output,
                    "last received"
                );
                final_terminals.mark_disconnected(&final_sid).await;
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        );
        write_data(&state, &terminals, &sid, "sending".into())
            .await
            .unwrap();
        entered_rx.await.unwrap();
        let first = request_shutdown(&state.sessions, &sid).await.unwrap();
        let second = shutdown_session(&state.sessions, &sid);
        tokio::pin!(second);
        assert!(futures::poll!(&mut second).is_pending());
        cleared_rx.await.unwrap();
        assert_eq!(handles.load(Ordering::SeqCst), 1);
        assert!(futures::poll!(&mut second).is_pending());
        finish_reader.send(()).unwrap();
        drain_output.send(()).unwrap();
        let result = second.await;
        assert_eq!(result.is_err(), purge_fails);
        assert_eq!(first.wait().await, result);
        assert_eq!(handles.load(Ordering::SeqCst), 0);
        assert_eq!(finalized.load(Ordering::SeqCst), 1);
        // A fresh handle can be acquired immediately after successful local cleanup.
        let reopened = HandleLease::new(&handles);
        assert_eq!(handles.load(Ordering::SeqCst), 1);
        drop(reopened);
    }
}
