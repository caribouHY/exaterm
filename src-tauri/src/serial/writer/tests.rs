use super::*;
use std::sync::Mutex;

struct TestWriter<F>(F);

impl<F: FnMut(&[u8]) -> io::Result<usize>> SerialWritePort for TestWriter<F> {}

impl<F: FnMut(&[u8]) -> io::Result<usize>> Write for TestWriter<F> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        (self.0)(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("disconnect must not flush queued output to the device")
    }
}

#[test]
fn stopped_writer_discards_large_queue_even_with_a_live_sender() {
    let (tx, rx) = mpsc::channel();
    for _ in 0..1024 {
        tx.send(vec![1; 4096]).unwrap();
    }
    let running = AtomicBool::new(false);
    let mut port = TestWriter(|_: &[u8]| panic!("stopped writer must not write"));
    assert_eq!(process_serial_writes(&mut port, rx, &running).unwrap(), 0);
    assert!(tx.send(vec![2]).is_err());
}

#[test]
fn partial_writes_advance_without_duplication_and_use_bounded_chunks() {
    let running = AtomicBool::new(true);
    let data = vec![42; WRITE_CHUNK_SIZE * 3 + 7];
    let (tx, rx) = mpsc::channel();
    tx.send(data.clone()).unwrap();
    drop(tx);
    let mut output = Vec::new();
    let mut port = TestWriter(|chunk: &[u8]| {
        assert!(chunk.len() <= WRITE_CHUNK_SIZE);
        let n = chunk.len().min(37);
        output.extend_from_slice(&chunk[..n]);
        Ok(n)
    });
    assert_eq!(
        process_serial_writes(&mut port, rx, &running).unwrap(),
        data.len()
    );
    assert_eq!(output, data);
}

#[test]
fn stopping_during_a_partial_write_discards_its_remainder_and_next_messages() {
    let running = AtomicBool::new(true);
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1; 8192]).unwrap();
    tx.send(vec![2; 8192]).unwrap();
    let mut calls = 0;
    let mut port = TestWriter(|_: &[u8]| {
        calls += 1;
        running.store(false, Ordering::SeqCst);
        Ok(2)
    });
    assert_eq!(process_serial_writes(&mut port, rx, &running).unwrap(), 2);
    assert_eq!(calls, 1);
    assert!(tx.send(vec![3]).is_err());
}

#[test]
fn zero_write_and_normal_io_errors_fail() {
    for error in [io::ErrorKind::WriteZero, io::ErrorKind::BrokenPipe] {
        let (tx, rx) = mpsc::channel();
        tx.send(vec![1]).unwrap();
        drop(tx);
        let mut port = TestWriter(|_: &[u8]| {
            if error == io::ErrorKind::WriteZero {
                Ok(0)
            } else {
                Err(error.into())
            }
        });
        assert_eq!(
            process_serial_writes(&mut port, rx, &AtomicBool::new(true))
                .unwrap_err()
                .kind(),
            error
        );
    }
}

#[test]
fn interrupted_write_retries_only_while_running() {
    for stop in [false, true] {
        let running = AtomicBool::new(true);
        let (tx, rx) = mpsc::channel();
        tx.send(vec![1, 2]).unwrap();
        drop(tx);
        let mut calls = 0;
        let mut port = TestWriter(|data: &[u8]| {
            calls += 1;
            if calls == 1 {
                if stop {
                    running.store(false, Ordering::SeqCst);
                }
                Err(io::ErrorKind::Interrupted.into())
            } else {
                Ok(data.len())
            }
        });
        assert_eq!(
            process_serial_writes(&mut port, rx, &running).unwrap(),
            if stop { 0 } else { 2 }
        );
        assert_eq!(calls, if stop { 1 } else { 2 });
    }
}

#[test]
fn stop_does_not_hide_unrelated_write_errors() {
    let running = AtomicBool::new(true);
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1]).unwrap();
    let mut port = TestWriter(|_: &[u8]| {
        running.store(false, Ordering::SeqCst);
        Err(io::ErrorKind::BrokenPipe.into())
    });
    assert_eq!(
        process_serial_writes(&mut port, rx, &running)
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[tokio::test]
async fn cancellation_repeats_if_stop_races_with_io_start() {
    let running = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1, 2, 3]).unwrap();
    drop(tx);
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let (begin_io, begin_io_rx) = mpsc::channel();
    let (abort_io, abort_io_rx) = mpsc::channel();
    let mut entered = Some(entered);
    let port = TestWriter(move |_: &[u8]| {
        entered.take().unwrap().send(()).unwrap();
        begin_io_rx.recv().unwrap();
        abort_io_rx.recv().unwrap();
        Err(io::ErrorKind::Interrupted.into())
    });
    let writer = spawn_serial_writer(port, rx, running.clone(), |_| {
        panic!("cancellation is not an error")
    })
    .unwrap();
    entered_rx.await.unwrap();
    running.store(false, Ordering::SeqCst);
    let mut calls = 0;
    stop_serial_writer(writer, |_| {
        calls += 1;
        if calls == 1 {
            // Simulate ERROR_NOT_FOUND before the actual I/O starts.
            begin_io.send(()).unwrap();
        } else {
            let _ = abort_io.send(());
        }
        Ok(())
    })
    .await
    .unwrap();
    assert!(calls >= 2);
}

#[tokio::test]
async fn cancellation_failure_is_reported_after_writer_release() {
    let (released, released_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        released_rx.recv().unwrap();
        Ok(0)
    });
    let result = stop_serial_writer(writer, |_| {
        let _ = released.send(());
        Err(io::ErrorKind::PermissionDenied.into())
    })
    .await;
    assert!(result.unwrap_err().contains("Failed to cancel Serial I/O"));
}

struct DropWriter(Arc<AtomicBool>);

impl SerialWritePort for DropWriter {}

impl Write for DropWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for DropWriter {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn failed_thread_spawn_releases_port_and_queue() {
    let dropped = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let result = spawn_serial_writer_with(
        DropWriter(dropped.clone()),
        rx,
        Arc::new(AtomicBool::new(true)),
        |_| panic!("worker never started"),
        |_| Err(io::ErrorKind::Other.into()),
    );
    assert!(result.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert!(tx.send(vec![1]).is_err());
}

#[tokio::test]
async fn writer_error_notification_follows_port_drop() {
    let dropped = Arc::new(AtomicBool::new(false));
    struct FailingWriter {
        _port: DropWriter,
    }
    impl SerialWritePort for FailingWriter {}
    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1]).unwrap();
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let notification = notifications.clone();
    let released = dropped.clone();
    let writer = spawn_serial_writer(
        FailingWriter {
            _port: DropWriter(dropped),
        },
        rx,
        Arc::new(AtomicBool::new(true)),
        move |error| {
            assert!(released.load(Ordering::SeqCst));
            notification.lock().unwrap().push(error);
        },
    )
    .unwrap();
    assert!(stop_serial_writer(writer, |_| Ok(())).await.is_err());
    assert_eq!(notifications.lock().unwrap().len(), 1);
}

#[cfg(windows)]
#[test]
fn cancelling_an_idle_dedicated_thread_accepts_error_not_found() {
    let (exit, exit_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        exit_rx.recv().unwrap();
        Ok(0)
    });
    cancel_synchronous_write(&writer).unwrap();
    exit.send(()).unwrap();
    writer.join().unwrap().unwrap();
}

#[cfg(windows)]
#[tokio::test]
async fn native_cancellation_stops_pending_synchronous_io_on_the_owned_thread() {
    use std::io::Read;
    use tokio::net::windows::named_pipe::ServerOptions;

    let name = format!(
        r"\\.\pipe\exaterm-serial-cancel-test-{}",
        uuid::Uuid::new_v4()
    );
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&name)
        .unwrap();
    server.connect().await.unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let observed = cancelled.clone();
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let mut entered = Some(entered);
    let mut file = file;
    // A pipe with no incoming data supplies deterministic pending synchronous I/O.
    let port = TestWriter(move |_: &[u8]| {
        entered.take().unwrap().send(()).unwrap();
        let result = file.read(&mut [0u8; 1]);
        if result
            .as_ref()
            .is_err_and(|error| error.raw_os_error() == Some(995))
        {
            observed.store(true, Ordering::SeqCst);
        }
        result
    });
    let running = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1]).unwrap();
    let writer = spawn_serial_writer(port, rx, running.clone(), |_| {
        panic!("cancelled I/O must not report an error")
    })
    .unwrap();
    entered_rx.await.unwrap();
    running.store(false, Ordering::SeqCst);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        stop_serial_writer(writer, cancel_synchronous_write),
    )
    .await;
    drop(server);
    result
        .expect("native cancellation must finish the owned thread")
        .unwrap();
    assert!(cancelled.load(Ordering::SeqCst));
}
