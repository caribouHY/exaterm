use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

mod timing;
pub(super) use timing::WritePolicy;

const WRITE_CHUNK_SIZE: usize = 4096;
const SERIAL_CANCEL_RETRY_INTERVAL: Duration = Duration::from_millis(5);

pub(super) trait SerialWritePort: Write {
    fn policy(&self) -> Option<WritePolicy> {
        None
    }

    fn set_write_timeout(&mut self, _timeout: Duration) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(not(windows))]
impl SerialWritePort for Box<dyn serialport::SerialPort> {}

pub(super) struct SerialWriter {
    thread: JoinHandle<io::Result<usize>>,
    deadlines: Arc<Mutex<TransmissionDeadlines>>,
}

#[derive(Default)]
struct TransmissionDeadlines {
    active: Option<Instant>,
    settle_until: Option<Instant>,
}

#[cfg(test)]
pub(super) fn process_serial_writes<W: SerialWritePort>(
    port: &mut W,
    queue: mpsc::Receiver<Vec<u8>>,
    running: &AtomicBool,
) -> io::Result<usize> {
    process_serial_writes_with_clock(port, queue, running, &Mutex::default(), Instant::now)
}

fn process_serial_writes_with_clock<W: SerialWritePort>(
    port: &mut W,
    queue: mpsc::Receiver<Vec<u8>>,
    running: &AtomicBool,
    deadlines: &Mutex<TransmissionDeadlines>,
    mut now: impl FnMut() -> Instant,
) -> io::Result<usize> {
    let policy = port.policy();
    let mut submitted = 0;
    while running.load(Ordering::SeqCst) {
        let Ok(data) = queue.recv() else {
            break;
        };
        let mut offset = 0;
        while offset < data.len() {
            if !running.load(Ordering::SeqCst) {
                return Ok(submitted);
            }
            let chunk_end = policy.map_or(data.len(), |policy| {
                data.len().min(offset + policy.chunk_size())
            });
            let deadline = policy.map(|policy| now() + policy.timeout(chunk_end - offset));
            while offset < chunk_end {
                if !running.load(Ordering::SeqCst) {
                    return Ok(submitted);
                }
                let result = if let Some(deadline) = deadline {
                    let remaining = deadline.saturating_duration_since(now());
                    if remaining.is_zero() {
                        return deadline_result(running, submitted);
                    }
                    match port.set_write_timeout(remaining) {
                        Ok(()) => {
                            if !running.load(Ordering::SeqCst) {
                                return Ok(submitted);
                            }
                            if now() >= deadline {
                                return deadline_result(running, submitted);
                            }
                            let Some(result) = write_if_running(
                                port,
                                &data[offset..chunk_end],
                                running,
                                deadlines,
                                Some(deadline),
                            ) else {
                                return Ok(submitted);
                            };
                            result
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    let Some(result) = write_if_running(
                        port,
                        &data[offset..data.len().min(offset + WRITE_CHUNK_SIZE)],
                        running,
                        deadlines,
                        None,
                    ) else {
                        return Ok(submitted);
                    };
                    result
                };
                if let Ok(written) = &result {
                    submitted += written;
                    offset += written;
                }
                // A cancelled write may still complete normally or return partial progress.
                if !running.load(Ordering::SeqCst) {
                    return match result {
                        Err(error) if !is_cancelled_io(&error) => Err(error),
                        _ => Ok(submitted),
                    };
                }
                match result {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    Ok(submitted)
}

fn write_if_running<W: Write>(
    port: &mut W,
    data: &[u8],
    running: &AtomicBool,
    deadlines: &Mutex<TransmissionDeadlines>,
    deadline: Option<Instant>,
) -> Option<io::Result<usize>> {
    let mut shared = deadlines.lock().unwrap_or_else(|error| error.into_inner());
    // Publish before releasing the lock: shutdown either observes this original deadline,
    // or closes acceptance before this writer can begin an operation.
    if !running.load(Ordering::SeqCst) {
        return None;
    }
    shared.active = deadline;
    drop(shared);
    let result = port.write(data);
    let mut shared = deadlines.lock().unwrap_or_else(|error| error.into_inner());
    // API completion can precede device-side settling, including between queued writes.
    // A later short write must not shorten an earlier successful block's hold budget.
    if result.as_ref().is_ok_and(|written| *written > 0) {
        shared.settle_until = shared.settle_until.max(deadline);
    }
    // Retain the cutoff operation's budget even if its result precedes the coordinator.
    if running.load(Ordering::SeqCst) {
        shared.active = None;
    }
    Some(result)
}

fn deadline_result(running: &AtomicBool, submitted: usize) -> io::Result<usize> {
    if running.load(Ordering::SeqCst) {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Serial transmission deadline exceeded",
        ))
    } else {
        Ok(submitted)
    }
}

fn is_cancelled_io(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Interrupted || cfg!(windows) && error.raw_os_error() == Some(995)
}

pub(super) fn spawn_serial_writer<W, Error>(
    port: W,
    queue: mpsc::Receiver<Vec<u8>>,
    running: Arc<AtomicBool>,
    on_error: Error,
) -> io::Result<SerialWriter>
where
    W: SerialWritePort + Send + 'static,
    Error: FnOnce(String) + Send + 'static,
{
    spawn_serial_writer_with(port, queue, running, on_error, |worker| {
        thread::Builder::new()
            .name("serial-writer".into())
            .spawn(worker)
    })
}

fn spawn_serial_writer_with<W, Error, Spawn>(
    mut port: W,
    queue: mpsc::Receiver<Vec<u8>>,
    running: Arc<AtomicBool>,
    on_error: Error,
    spawn: Spawn,
) -> io::Result<SerialWriter>
where
    W: SerialWritePort + Send + 'static,
    Error: FnOnce(String) + Send + 'static,
    Spawn: FnOnce(
        Box<dyn FnOnce() -> io::Result<usize> + Send>,
    ) -> io::Result<JoinHandle<io::Result<usize>>>,
{
    let deadlines = Arc::new(Mutex::default());
    let worker_deadlines = deadlines.clone();
    let thread = spawn(Box::new(move || {
        let result = process_serial_writes_with_clock(
            &mut port,
            queue,
            &running,
            &worker_deadlines,
            Instant::now,
        );
        drop(port);
        if let Err(error) = &result {
            on_error(error.to_string());
        }
        result
    }))?;
    Ok(SerialWriter { thread, deadlines })
}

pub(super) async fn stop_serial_writer<Cancel>(
    writer: SerialWriter,
    cancel: Cancel,
) -> Result<(), String>
where
    Cancel: FnMut(&JoinHandle<io::Result<usize>>) -> io::Result<()>,
{
    stop_serial_writer_with_clock(writer, cancel, Instant::now).await
}

async fn stop_serial_writer_with_clock<Cancel>(
    writer: SerialWriter,
    mut cancel: Cancel,
    mut now: impl FnMut() -> Instant,
) -> Result<(), String>
where
    Cancel: FnMut(&JoinHandle<io::Result<usize>>) -> io::Result<()>,
{
    let (deadline, settle_until) = {
        let shared = writer
            .deadlines
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (shared.active, shared.settle_until.max(shared.active))
    };
    // Cancelling a healthy synchronous FTDI write can leave the first reopen unresponsive.
    // Reuse its existing budget, never a fresh timeout measured from the disconnect request.
    let mut cancellation_error = None;
    while !writer.thread.is_finished() {
        if deadline.is_none_or(|deadline| now() >= deadline) {
            if let Err(error) = cancel(&writer.thread) {
                cancellation_error
                    .get_or_insert_with(|| format!("Failed to cancel Serial I/O: {error}"));
            }
        }
        tokio::time::sleep(SERIAL_CANCEL_RETRY_INTERVAL).await;
    }
    let result = writer
        .thread
        .join()
        .map_err(|_| "The Serial writer panicked".to_string())?
        .map(|_| ())
        .map_err(|error| format!("Serial write failed: {error}"));
    if result.is_ok() && cancellation_error.is_none() {
        // API/flush success alone did not make immediate reuse reliable on the tested FTDI
        // device. The cleanup caller retains the control handle for the original budgets,
        // including successful writes that finished before the disconnect request.
        while settle_until.is_some_and(|deadline| now() < deadline) {
            tokio::time::sleep(SERIAL_CANCEL_RETRY_INTERVAL).await;
        }
    }
    cancellation_error.map_or(result, Err)
}

pub(super) async fn finish_serial_workers<Cancel, Clear>(
    writer: SerialWriter,
    cancel: Cancel,
    clear_output: Clear,
    reader: tokio::task::JoinHandle<()>,
    output: tokio::task::JoinHandle<()>,
) -> Result<(), String>
where
    Cancel: FnMut(&JoinHandle<io::Result<usize>>) -> io::Result<()>,
    Clear: FnOnce() -> Result<(), String> + Send + 'static,
{
    let write_result = stop_serial_writer(writer, cancel).await;
    let clear_result = tokio::task::spawn_blocking(clear_output)
        .await
        .map_err(|error| format!("Serial output cleanup failed: {error}"))
        .and_then(|result| result);
    let read_result = reader
        .await
        .map_err(|error| format!("Serial reader failed: {error}"));
    let output_result = output
        .await
        .map_err(|error| format!("Serial output worker failed: {error}"));
    write_result
        .and(clear_result)
        .and(read_result)
        .and(output_result)
}

#[cfg(windows)]
pub(super) fn cancel_synchronous_write(writer: &JoinHandle<io::Result<usize>>) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
    use windows_sys::Win32::System::IO::CancelSynchronousIo;

    // SAFETY: The borrowed JoinHandle keeps this OS thread handle open throughout the call.
    // It belongs to a dedicated writer, so cancellation cannot affect a reused pool thread.
    // The API requests I/O cancellation without terminating the thread or accessing Rust memory;
    // the coordinator retains the handle and joins only after the writer has exited.
    if unsafe { CancelSynchronousIo(writer.as_raw_handle()) } != 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(not(windows))]
pub(super) fn cancel_synchronous_write(_writer: &JoinHandle<io::Result<usize>>) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod deadline_tests;
#[cfg(test)]
mod shutdown_tests;
#[cfg(test)]
mod tests;
