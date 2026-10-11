use super::*;
use std::sync::atomic::AtomicUsize;

struct PendingWriter {
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: mpsc::Receiver<()>,
    calls: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
    result: Option<io::ErrorKind>,
}

impl SerialWritePort for PendingWriter {
    fn policy(&self) -> Option<WritePolicy> {
        Some(test_policy())
    }
}

fn test_policy() -> WritePolicy {
    WritePolicy::new(
        9600,
        serialport::DataBits::Eight,
        serialport::Parity::None,
        serialport::StopBits::One,
    )
    .unwrap()
}

impl Write for PendingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        assert_eq!(self.calls.fetch_add(1, Ordering::SeqCst), 0);
        self.entered.take().unwrap().send(()).unwrap();
        self.release.recv().unwrap();
        self.result.map_or(Ok(2), |error| Err(error.into()))
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("shutdown must not flush queued bytes")
    }
}

impl Drop for PendingWriter {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn grace_cleanup_survives_caller_abort_and_shares_completion_after_all_workers() {
    use crate::serial::lifecycle::{spawn_shutdown_coordinator, SerialSession, SerialShutdown};
    use crate::serial::{shutdown_session, SerialState};
    use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};

    let state = SerialState::new();
    let terminals = TerminalControlState::new();
    let shutdown = SerialShutdown::new();
    let sid = uuid::Uuid::new_v4().to_string();
    terminals
        .register_session(sid.clone(), TerminalProtocol::Serial, "COM1".into())
        .await;
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1; 8192]).unwrap();
    tx.send(vec![2; 8192]).unwrap();
    state.sessions.lock().await.insert(
        sid.clone(),
        SerialSession {
            shutdown: shutdown.clone(),
            writer: Some(tx),
        },
    );
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = mpsc::channel();
    let dropped = Arc::new(AtomicBool::new(false));
    let writer = spawn_serial_writer(
        PendingWriter {
            entered: Some(entered),
            release: release_rx,
            calls: Arc::new(AtomicUsize::new(0)),
            dropped: dropped.clone(),
            result: None,
        },
        rx,
        shutdown.running.clone(),
        |_| panic!("healthy shutdown must not fail"),
    )
    .unwrap();
    entered_rx.await.unwrap();
    let (cleared, cleared_rx) = tokio::sync::oneshot::channel();
    let (read_done, read_done_rx) = tokio::sync::oneshot::channel();
    let reader = tokio::spawn(async move {
        read_done_rx.await.unwrap();
    });
    let (output_done, output_done_rx) = tokio::sync::oneshot::channel();
    let retained = terminals.clone();
    let output_sid = sid.clone();
    let output = tokio::spawn(async move {
        output_done_rx.await.unwrap();
        retained.append_output(&output_sid, b"last received").await;
    });
    let cleanup = finish_serial_workers(
        writer,
        |_| panic!("pending write must finish without forced cancellation"),
        move || {
            cleared.send(()).unwrap();
            Ok(())
        },
        reader,
        output,
    );
    let finalized = Arc::new(AtomicUsize::new(0));
    let count = finalized.clone();
    let final_terminals = terminals.clone();
    let final_sid = sid.clone();
    let released = dropped.clone();
    spawn_shutdown_coordinator(
        state.sessions.clone(),
        sid.clone(),
        shutdown.clone(),
        cleanup,
        move || async move {
            assert!(released.load(Ordering::SeqCst));
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
    let first = tokio::spawn({
        let sessions = state.sessions.clone();
        let session_id = sid.clone();
        async move { shutdown_session(&sessions, &session_id).await }
    });
    while shutdown.running.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    let second = shutdown_session(&state.sessions, &sid);
    tokio::pin!(second);
    assert!(futures::poll!(&mut second).is_pending());
    assert!(!dropped.load(Ordering::SeqCst));
    assert_eq!(finalized.load(Ordering::SeqCst), 0);
    release.send(()).unwrap();
    cleared_rx.await.unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    assert!(futures::poll!(&mut second).is_pending());
    assert_eq!(
        terminals.session_info(&sid).await.unwrap().status,
        TerminalStatus::Connected
    );
    read_done.send(()).unwrap();
    output_done.send(()).unwrap();
    second.await.unwrap();
    shutdown.wait().await.unwrap();
    assert_eq!(finalized.load(Ordering::SeqCst), 1);
    assert!(!state.sessions.lock().await.contains_key(&sid));
    assert_eq!(
        terminals.session_info(&sid).await.unwrap().status,
        TerminalStatus::Disconnected
    );
}

#[tokio::test]
async fn shutdown_allows_pending_io_to_finish_and_discards_every_remainder() {
    for error in [
        None,
        Some(io::ErrorKind::Interrupted),
        Some(io::ErrorKind::BrokenPipe),
    ] {
        let (tx, rx) = mpsc::channel();
        tx.send(vec![1; 8192]).unwrap();
        tx.send(vec![2; 8192]).unwrap();
        let (entered, entered_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let calls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let writer = spawn_serial_writer(
            PendingWriter {
                entered: Some(entered),
                release: release_rx,
                calls: calls.clone(),
                dropped: dropped.clone(),
                result: error,
            },
            rx,
            running.clone(),
            |_| {},
        )
        .unwrap();
        entered_rx.await.unwrap();
        running.store(false, Ordering::SeqCst);
        let deadline = writer.deadlines.lock().unwrap().active.unwrap();
        let clock = Arc::new(Mutex::new(deadline - Duration::from_millis(1)));
        let current = clock.clone();
        let stop = stop_serial_writer_with_clock(
            writer,
            |_| panic!("healthy pending write must not be cancelled"),
            move || *current.lock().unwrap(),
        );
        tokio::pin!(stop);
        assert!(futures::poll!(&mut stop).is_pending());
        assert!(!dropped.load(Ordering::SeqCst));
        release.send(()).unwrap();
        while !dropped.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        if error != Some(io::ErrorKind::BrokenPipe) {
            assert!(futures::poll!(&mut stop).is_pending());
            *clock.lock().unwrap() = deadline;
        }
        let result = tokio::time::timeout(Duration::from_secs(1), stop)
            .await
            .unwrap();
        assert_eq!(result.is_err(), error == Some(io::ErrorKind::BrokenPipe));
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(tx.send(vec![3]).is_err());
    }
}

#[tokio::test]
async fn cutoff_deadline_survives_normal_completion_before_coordinator_snapshot() {
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1; 8192]).unwrap();
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    let writer = spawn_serial_writer(
        PendingWriter {
            entered: Some(entered),
            release: release_rx,
            calls: Arc::new(AtomicUsize::new(0)),
            dropped: Arc::new(AtomicBool::new(false)),
            result: None,
        },
        rx,
        running.clone(),
        |_| panic!("normal completion must not fail"),
    )
    .unwrap();
    entered_rx.await.unwrap();
    let deadline = writer.deadlines.lock().unwrap().active.unwrap();
    running.store(false, Ordering::SeqCst);
    release.send(()).unwrap();
    while !writer.thread.is_finished() {
        tokio::task::yield_now().await;
    }
    assert_eq!(writer.deadlines.lock().unwrap().active, Some(deadline));
    let clock = Arc::new(Mutex::new(deadline - Duration::from_millis(1)));
    let current = clock.clone();
    let stop = stop_serial_writer_with_clock(
        writer,
        |_| panic!("finished thread must not be cancelled"),
        move || *current.lock().unwrap(),
    );
    tokio::pin!(stop);
    assert!(futures::poll!(&mut stop).is_pending());
    *clock.lock().unwrap() = deadline;
    tokio::time::timeout(Duration::from_secs(1), stop)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn normally_finished_idle_writer_holds_only_the_remaining_original_budget() {
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1, 2]).unwrap();
    drop(tx);
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    let writer = spawn_serial_writer(
        PendingWriter {
            entered: Some(entered),
            release: release_rx,
            calls: Arc::new(AtomicUsize::new(0)),
            dropped: Arc::new(AtomicBool::new(false)),
            result: None,
        },
        rx,
        running.clone(),
        |_| panic!("normal completion must not fail"),
    )
    .unwrap();
    entered_rx.await.unwrap();
    let deadline = writer.deadlines.lock().unwrap().active.unwrap();
    release.send(()).unwrap();
    while !writer.thread.is_finished() {
        tokio::task::yield_now().await;
    }
    assert!(writer.deadlines.lock().unwrap().active.is_none());
    assert_eq!(
        writer.deadlines.lock().unwrap().settle_until,
        Some(deadline)
    );
    running.store(false, Ordering::SeqCst);
    let clock = Arc::new(Mutex::new(deadline - Duration::from_millis(1)));
    let current = clock.clone();
    let stop = stop_serial_writer_with_clock(
        writer,
        |_| panic!("idle finished writer must not be cancelled"),
        move || *current.lock().unwrap(),
    );
    tokio::pin!(stop);
    assert!(futures::poll!(&mut stop).is_pending());
    *clock.lock().unwrap() = deadline;
    tokio::time::timeout(Duration::from_secs(1), stop)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn shorter_pending_write_keeps_its_cancellation_deadline_and_prior_settling_budget() {
    use super::deadline_tests::TimedWriter;
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let mut entered = Some(entered);
    let (release, release_rx) = mpsc::channel();
    let mut calls = 0;
    let port = TimedWriter {
        policy: test_policy(),
        set_timeout: |_| Ok(()),
        write: move |data: &[u8]| {
            calls += 1;
            if calls == 1 {
                assert_eq!(data.len(), 4096);
                return Ok(data.len());
            }
            assert_eq!(calls, 2);
            assert_eq!(data.len(), 1);
            entered.take().unwrap().send(()).unwrap();
            release_rx.recv().unwrap();
            Err(io::ErrorKind::Interrupted.into())
        },
    };
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1; 4096]).unwrap();
    tx.send(vec![2]).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let writer = spawn_serial_writer(port, rx, running.clone(), |_| {
        panic!("cancel is not an error")
    })
    .unwrap();
    entered_rx.await.unwrap();
    let (active, settling) = {
        let shared = writer.deadlines.lock().unwrap();
        (shared.active.unwrap(), shared.settle_until.unwrap())
    };
    assert!(active < settling);
    running.store(false, Ordering::SeqCst);
    let clock = Arc::new(Mutex::new(active - Duration::from_millis(1)));
    let current = clock.clone();
    let cancellations = Arc::new(AtomicUsize::new(0));
    let observed = cancellations.clone();
    let stop = stop_serial_writer_with_clock(
        writer,
        move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            let _ = release.send(());
            Ok(())
        },
        move || *current.lock().unwrap(),
    );
    tokio::pin!(stop);
    assert!(futures::poll!(&mut stop).is_pending());
    assert_eq!(cancellations.load(Ordering::SeqCst), 0);
    *clock.lock().unwrap() = active;
    while cancellations.load(Ordering::SeqCst) == 0 {
        assert!(futures::poll!(&mut stop).is_pending());
        tokio::time::sleep(SERIAL_CANCEL_RETRY_INTERVAL).await;
    }
    assert!(futures::poll!(&mut stop).is_pending());
    *clock.lock().unwrap() = settling;
    tokio::time::timeout(Duration::from_secs(1), stop)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn expired_settling_budget_does_not_start_a_new_idle_wait() {
    let deadline = Instant::now();
    let writer = SerialWriter {
        thread: thread::spawn(|| Ok(0)),
        deadlines: Arc::new(Mutex::new(TransmissionDeadlines {
            active: None,
            settle_until: Some(deadline),
        })),
    };
    while !writer.thread.is_finished() {
        tokio::task::yield_now().await;
    }
    tokio::time::timeout(
        Duration::from_secs(1),
        stop_serial_writer_with_clock(
            writer,
            |_| panic!("finished writer must not be cancelled"),
            || deadline,
        ),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test]
async fn shutdown_cancels_at_the_original_deadline_without_starting_a_new_budget() {
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1; 8192]).unwrap();
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    let dropped = Arc::new(AtomicBool::new(false));
    let writer = spawn_serial_writer(
        PendingWriter {
            entered: Some(entered),
            release: release_rx,
            calls: Arc::new(AtomicUsize::new(0)),
            dropped: dropped.clone(),
            result: Some(io::ErrorKind::Interrupted),
        },
        rx,
        running.clone(),
        |_| panic!("cancellation is not a write error"),
    )
    .unwrap();
    entered_rx.await.unwrap();
    let deadline = writer.deadlines.lock().unwrap().active.unwrap();
    let clock = Arc::new(Mutex::new(deadline - Duration::from_millis(1)));
    let current = clock.clone();
    let cancellations = Arc::new(AtomicUsize::new(0));
    let observed = cancellations.clone();
    running.store(false, Ordering::SeqCst);
    let stop = stop_serial_writer_with_clock(
        writer,
        move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            let _ = release.send(());
            Ok(())
        },
        move || *current.lock().unwrap(),
    );
    tokio::pin!(stop);
    assert!(futures::poll!(&mut stop).is_pending());
    assert_eq!(cancellations.load(Ordering::SeqCst), 0);
    *clock.lock().unwrap() = deadline;
    tokio::time::timeout(Duration::from_secs(1), stop)
        .await
        .unwrap()
        .unwrap();
    assert!(cancellations.load(Ordering::SeqCst) >= 1);
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn partial_and_interrupted_io_publish_the_same_block_deadline() {
    use super::deadline_tests::TimedWriter;
    use std::cell::Cell;
    for interrupted in [false, true] {
        let start = Instant::now();
        let clock = Cell::new(start);
        let pending = Mutex::<TransmissionDeadlines>::default();
        let calls = Cell::new(0);
        let mut port = TimedWriter {
            policy: test_policy(),
            set_timeout: |_| Ok(()),
            write: |_: &[u8]| {
                assert_eq!(
                    pending.lock().unwrap().active,
                    Some(start + Duration::from_secs(5))
                );
                calls.set(calls.get() + 1);
                clock.set(clock.get() + Duration::from_secs(2));
                if interrupted {
                    Err(io::ErrorKind::Interrupted.into())
                } else {
                    Ok(1)
                }
            },
        };
        let (tx, rx) = mpsc::channel();
        tx.send(vec![1; 10]).unwrap();
        drop(tx);
        assert_eq!(
            process_serial_writes_with_clock(
                &mut port,
                rx,
                &AtomicBool::new(true),
                &pending,
                || clock.get(),
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(calls.get(), 3);
        assert!(pending.lock().unwrap().active.is_none());
    }
}

#[tokio::test]
async fn shutdown_during_timeout_configuration_prevents_late_io_submission() {
    use super::deadline_tests::TimedWriter;
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let mut entered = Some(entered);
    let (release, release_rx) = mpsc::channel();
    let port = TimedWriter {
        policy: test_policy(),
        write: |_: &[u8]| -> io::Result<usize> { panic!("shutdown cutoff forbids late write") },
        set_timeout: move |_| {
            entered.take().unwrap().send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        },
    };
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1; 8192]).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let writer = spawn_serial_writer(port, rx, running.clone(), |_| {
        panic!("shutdown is not an error")
    })
    .unwrap();
    entered_rx.await.unwrap();
    running.store(false, Ordering::SeqCst);
    assert!(writer.deadlines.lock().unwrap().active.is_none());
    stop_serial_writer(writer, |_| {
        let _ = release.send(());
        Ok(())
    })
    .await
    .unwrap();
    assert!(tx.send(vec![2]).is_err());
}
