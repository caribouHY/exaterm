use super::*;
use std::cell::Cell;

pub(super) struct TimedWriter<W, S> {
    pub(super) write: W,
    pub(super) set_timeout: S,
    pub(super) policy: WritePolicy,
}

impl<W: FnMut(&[u8]) -> io::Result<usize>, S> Write for TimedWriter<W, S> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        (self.write)(data)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("shutdown must not flush unsent data")
    }
}

impl<W, S> SerialWritePort for TimedWriter<W, S>
where
    W: FnMut(&[u8]) -> io::Result<usize>,
    S: FnMut(Duration) -> io::Result<()>,
{
    fn policy(&self) -> Option<WritePolicy> {
        Some(self.policy)
    }
    fn set_write_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        (self.set_timeout)(timeout)
    }
}

fn policy(baud: u32) -> WritePolicy {
    WritePolicy::new(
        baud,
        serialport::DataBits::Eight,
        serialport::Parity::None,
        serialport::StopBits::One,
    )
    .unwrap()
}

fn run<W: SerialWritePort>(
    port: &mut W,
    data: Vec<u8>,
    running: &AtomicBool,
    now: impl FnMut() -> Instant,
) -> io::Result<usize> {
    let (tx, rx) = mpsc::channel();
    tx.send(data).unwrap();
    drop(tx);
    process_serial_writes_with_clock(port, rx, running, &Mutex::default(), now)
}

#[test]
fn partial_writes_keep_fixed_boundaries_and_consume_the_same_deadline() {
    let clock = Cell::new(Instant::now());
    let data: Vec<u8> = (0..8195).map(|i| (i % 251) as u8).collect();
    let mut output = Vec::new();
    let mut timeouts = Vec::new();
    let mut block_starts = Vec::new();
    let mut port = TimedWriter {
        write: |bytes: &[u8]| {
            let offset = output.len();
            let end = data.len().min((offset / 4096 + 1) * 4096);
            assert_eq!(bytes.len(), end - offset);
            if offset % 4096 == 0 {
                block_starts.push(offset);
            }
            let n = bytes.len().min(37);
            output.extend_from_slice(&bytes[..n]);
            clock.set(clock.get() + Duration::from_millis(20));
            Ok(n)
        },
        set_timeout: |timeout| {
            timeouts.push(timeout);
            Ok(())
        },
        policy: policy(115200),
    };
    assert_eq!(
        run(&mut port, data.clone(), &AtomicBool::new(true), || clock
            .get())
        .unwrap(),
        data.len()
    );
    drop(port);
    assert_eq!(output, data);
    let calls_per_block = 4096usize.div_ceil(37);
    for start in [0, calls_per_block, calls_per_block * 2] {
        assert_eq!(timeouts[start], Duration::from_secs(5));
    }
    assert_eq!(timeouts[1], Duration::from_millis(4980));
    assert_eq!(block_starts, [0, 4096, 8192]);
}

#[test]
fn low_baud_blocks_split_without_extending_their_budget() {
    let clock = Cell::new(Instant::now());
    let mut sizes = Vec::new();
    let mut timeouts = Vec::new();
    let mut port = TimedWriter {
        write: |data: &[u8]| {
            sizes.push(data.len());
            Ok(data.len())
        },
        set_timeout: |timeout| {
            timeouts.push(timeout);
            Ok(())
        },
        policy: policy(300),
    };
    assert_eq!(
        run(&mut port, vec![1; 1000], &AtomicBool::new(true), || clock
            .get())
        .unwrap(),
        1000
    );
    drop(port);
    assert_eq!(sizes, [435, 435, 130]);
    assert_eq!(
        timeouts,
        [
            Duration::from_secs(30),
            Duration::from_secs(30),
            Duration::from_millis(9667)
        ]
    );
}

#[test]
fn partial_and_interrupted_writes_do_not_restart_the_deadline() {
    for interrupted in [false, true] {
        let clock = Cell::new(Instant::now());
        let calls = Cell::new(0);
        let mut port = TimedWriter {
            write: |_: &[u8]| {
                calls.set(calls.get() + 1);
                clock.set(clock.get() + Duration::from_secs(2));
                if interrupted {
                    Err(io::ErrorKind::Interrupted.into())
                } else {
                    Ok(1)
                }
            },
            set_timeout: |_| Ok(()),
            policy: policy(115200),
        };
        assert_eq!(
            run(&mut port, vec![1; 10], &AtomicBool::new(true), || clock
                .get())
            .unwrap_err()
            .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(calls.get(), 3);
    }
}

#[test]
fn completed_blocks_succeed_even_when_the_result_arrives_after_the_deadline() {
    let clock = Cell::new(Instant::now());
    let mut timeouts = Vec::new();
    let mut port = TimedWriter {
        write: |bytes: &[u8]| {
            clock.set(clock.get() + Duration::from_secs(6));
            Ok(bytes.len())
        },
        set_timeout: |timeout| {
            timeouts.push(timeout);
            Ok(())
        },
        policy: policy(115200),
    };
    assert_eq!(
        run(&mut port, vec![1; 8192], &AtomicBool::new(true), || clock
            .get())
        .unwrap(),
        8192
    );
    drop(port);
    assert_eq!(timeouts, [Duration::from_secs(5), Duration::from_secs(5)]);
}

#[test]
fn shutdown_at_deadline_keeps_partial_progress_and_normal_errors() {
    for error in [
        None,
        Some(io::ErrorKind::Interrupted),
        Some(io::ErrorKind::BrokenPipe),
    ] {
        let clock = Cell::new(Instant::now());
        let running = AtomicBool::new(true);
        let mut port = TimedWriter {
            write: |_: &[u8]| {
                clock.set(clock.get() + Duration::from_secs(6));
                running.store(false, Ordering::SeqCst);
                error.map_or(Ok(2), |error| Err(error.into()))
            },
            set_timeout: |_| Ok(()),
            policy: policy(115200),
        };
        let result = run(&mut port, vec![1; 10], &running, || clock.get());
        if error == Some(io::ErrorKind::BrokenPipe) {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        } else {
            assert_eq!(result.unwrap(), if error.is_none() { 2 } else { 0 });
        }
    }
}

#[test]
fn timeout_configuration_failure_and_expiry_prevent_io() {
    for fail in [false, true] {
        let clock = Cell::new(Instant::now());
        let mut port = TimedWriter {
            write: |_: &[u8]| -> io::Result<usize> {
                panic!("unconfigured or expired write must not start")
            },
            set_timeout: |_| {
                if fail {
                    Err(io::ErrorKind::PermissionDenied.into())
                } else {
                    clock.set(clock.get() + Duration::from_secs(5));
                    Ok(())
                }
            },
            policy: policy(115200),
        };
        assert_eq!(
            run(&mut port, vec![1], &AtomicBool::new(true), || clock.get())
                .unwrap_err()
                .kind(),
            if fail {
                io::ErrorKind::PermissionDenied
            } else {
                io::ErrorKind::TimedOut
            }
        );
    }
}

#[test]
fn shutdown_observed_by_expiry_does_not_create_a_timeout_error() {
    let running = AtomicBool::new(true);
    let clock = Cell::new(Instant::now());
    let mut port = TimedWriter {
        write: |_: &[u8]| -> io::Result<usize> { panic!("stopped write must not start") },
        set_timeout: |_| Ok(()),
        policy: policy(115200),
    };
    let mut calls = 0;
    assert_eq!(
        run(&mut port, vec![1], &running, || {
            calls += 1;
            if calls == 2 {
                clock.set(clock.get() + Duration::from_secs(5));
                running.store(false, Ordering::SeqCst);
            }
            clock.get()
        })
        .unwrap(),
        0
    );
}

#[test]
fn zero_bytes_and_ordinary_errors_remain_observable_with_deadlines() {
    for error in [None, Some(io::ErrorKind::BrokenPipe)] {
        let mut port = TimedWriter {
            write: |_: &[u8]| error.map_or(Ok(0), |error| Err(error.into())),
            set_timeout: |_| Ok(()),
            policy: policy(115200),
        };
        assert_eq!(
            run(&mut port, vec![1], &AtomicBool::new(true), Instant::now)
                .unwrap_err()
                .kind(),
            error.unwrap_or(io::ErrorKind::WriteZero)
        );
    }
}
