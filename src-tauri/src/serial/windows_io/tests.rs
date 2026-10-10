use super::*;
use crate::serial::writer::{process_serial_writes, spawn_serial_writer, stop_serial_writer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

struct FakePort {
    timeouts: Arc<Mutex<COMMTIMEOUTS>>,
    failure: Option<bool>,
    writes: Arc<Mutex<Vec<u8>>>,
    released: Arc<AtomicBool>,
}

impl Write for FakePort {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.writes.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("disconnect must not transmit queued bytes")
    }
}

impl SerialTimeoutPort for FakePort {
    fn get_timeouts(&self) -> io::Result<COMMTIMEOUTS> {
        if self.failure == Some(false) {
            Err(io::ErrorKind::PermissionDenied.into())
        } else {
            Ok(*self.timeouts.lock().unwrap())
        }
    }
    fn set_timeouts(&mut self, timeouts: &COMMTIMEOUTS) -> io::Result<()> {
        if self.failure == Some(true) {
            Err(io::ErrorKind::PermissionDenied.into())
        } else {
            *self.timeouts.lock().unwrap() = *timeouts;
            Ok(())
        }
    }
}

impl Drop for FakePort {
    fn drop(&mut self) {
        self.released.store(true, Ordering::SeqCst);
    }
}

fn fixture(failure: Option<bool>) -> WindowsWriterPort<FakePort> {
    WindowsWriterPort {
        port: FakePort {
            timeouts: Arc::new(Mutex::new(COMMTIMEOUTS {
                ReadIntervalTimeout: u32::MAX,
                ReadTotalTimeoutMultiplier: u32::MAX,
                ReadTotalTimeoutConstant: 5,
                WriteTotalTimeoutMultiplier: 99,
                WriteTotalTimeoutConstant: 5,
            })),
            writes: Arc::new(Mutex::new(Vec::new())),
            failure,
            released: Arc::new(AtomicBool::new(false)),
        },
        policy: WritePolicy::new(
            115200,
            serialport::DataBits::Eight,
            serialport::Parity::None,
            serialport::StopBits::One,
        )
        .unwrap(),
    }
}

#[test]
fn production_writer_updates_transmission_without_changing_shared_receive_timeouts() {
    let mut port = fixture(None);
    let shared_timeouts = port.port.timeouts.clone();
    let (tx, rx) = mpsc::channel();
    let data = vec![42; 16384];
    tx.send(data.clone()).unwrap();
    drop(tx);
    assert_eq!(
        process_serial_writes(&mut port, rx, &AtomicBool::new(true)).unwrap(),
        data.len()
    );
    assert_eq!(*port.port.writes.lock().unwrap(), data);
    let timeouts = shared_timeouts.lock().unwrap();
    assert_eq!(timeouts.ReadIntervalTimeout, u32::MAX);
    assert_eq!(timeouts.ReadTotalTimeoutMultiplier, u32::MAX);
    assert_eq!(timeouts.ReadTotalTimeoutConstant, 5);
    assert_eq!(timeouts.WriteTotalTimeoutMultiplier, 0);
    assert!((1..=5000).contains(&timeouts.WriteTotalTimeoutConstant));
}

#[test]
fn fractional_milliseconds_round_up_and_never_select_infinite_timeout() {
    let mut port = fixture(None);
    for (duration, expected) in [
        (Duration::ZERO, 1),
        (Duration::from_nanos(1), 1),
        (Duration::from_micros(1001), 2),
        (Duration::from_secs(30), 30000),
        (Duration::MAX, u32::MAX - 1),
    ] {
        port.set_write_timeout(duration).unwrap();
        assert_eq!(
            port.port.timeouts.lock().unwrap().WriteTotalTimeoutConstant,
            expected
        );
    }
}

#[tokio::test]
async fn native_timeout_boundary_failure_releases_writer_before_error_notification() {
    for failure in [false, true] {
        let port = fixture(Some(failure));
        let released = port.port.released.clone();
        let writes = port.port.writes.clone();
        let observed = Arc::new(AtomicBool::new(false));
        let on_error_observed = observed.clone();
        let (tx, rx) = mpsc::channel();
        tx.send(vec![1]).unwrap();
        drop(tx);
        let writer = spawn_serial_writer(port, rx, Arc::new(AtomicBool::new(true)), move |_| {
            assert!(released.load(Ordering::SeqCst));
            on_error_observed.store(true, Ordering::SeqCst);
        })
        .unwrap();
        assert!(stop_serial_writer(writer, |_| Ok(())).await.is_err());
        assert!(observed.load(Ordering::SeqCst));
        assert!(writes.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn deadline_enabled_adapter_preserves_native_writer_cancellation() {
    use std::io::Read;
    use tokio::net::windows::named_pipe::ServerOptions;

    struct PendingPort {
        file: std::fs::File,
        entered: Option<tokio::sync::oneshot::Sender<()>>,
        timeouts: COMMTIMEOUTS,
        configured: Arc<AtomicBool>,
        cancelled: Arc<AtomicBool>,
        released: Arc<AtomicBool>,
    }
    impl Write for PendingPort {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            assert!(self.configured.load(Ordering::SeqCst));
            self.entered.take().unwrap().send(()).unwrap();
            let result = self.file.read(&mut [0u8; 1]);
            if result
                .as_ref()
                .is_err_and(|error| error.raw_os_error() == Some(995))
            {
                self.cancelled.store(true, Ordering::SeqCst);
            }
            result
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("disconnect must not flush")
        }
    }
    impl SerialTimeoutPort for PendingPort {
        fn get_timeouts(&self) -> io::Result<COMMTIMEOUTS> {
            Ok(self.timeouts)
        }
        fn set_timeouts(&mut self, timeouts: &COMMTIMEOUTS) -> io::Result<()> {
            assert_eq!(timeouts.ReadTotalTimeoutConstant, 5);
            assert_eq!(timeouts.WriteTotalTimeoutMultiplier, 0);
            assert!((1..=5000).contains(&timeouts.WriteTotalTimeoutConstant));
            self.timeouts = *timeouts;
            self.configured.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    impl Drop for PendingPort {
        fn drop(&mut self) {
            self.released.store(true, Ordering::SeqCst);
        }
    }

    let name = format!(
        r"\\.\pipe\exaterm-serial-deadline-cancel-{}",
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
    let (entered, entered_rx) = tokio::sync::oneshot::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let released = Arc::new(AtomicBool::new(false));
    let port = WindowsWriterPort {
        port: PendingPort {
            file,
            entered: Some(entered),
            timeouts: COMMTIMEOUTS {
                ReadTotalTimeoutConstant: 5,
                ..COMMTIMEOUTS::default()
            },
            configured: Arc::new(AtomicBool::new(false)),
            cancelled: cancelled.clone(),
            released: released.clone(),
        },
        policy: fixture(None).policy,
    };
    let (tx, rx) = mpsc::channel();
    tx.send(vec![1]).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let worker = spawn_serial_writer(port, rx, running.clone(), |_| {
        panic!("cancelled write must not report an error")
    })
    .unwrap();
    entered_rx.await.unwrap();
    running.store(false, Ordering::SeqCst);
    tokio::time::timeout(
        Duration::from_secs(2),
        stop_serial_writer(worker, crate::serial::writer::cancel_synchronous_write),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(cancelled.load(Ordering::SeqCst));
    assert!(released.load(Ordering::SeqCst));
    drop(server);
}
