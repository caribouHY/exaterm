use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};

use super::SERIAL_IO_TIMEOUT;

const WRITE_CHUNK_SIZE: usize = 4096;

pub(super) fn process_serial_writes<W: Write>(
    port: &mut W,
    queue: mpsc::Receiver<Vec<u8>>,
    running: &AtomicBool,
) -> io::Result<usize> {
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
            let end = data.len().min(offset + WRITE_CHUNK_SIZE);
            let result = port.write(&data[offset..end]);
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
    Ok(submitted)
}

fn is_cancelled_io(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Interrupted || cfg!(windows) && error.raw_os_error() == Some(995)
}

pub(super) fn spawn_serial_writer<W, Error>(
    port: W,
    queue: mpsc::Receiver<Vec<u8>>,
    running: Arc<AtomicBool>,
    on_error: Error,
) -> io::Result<JoinHandle<io::Result<usize>>>
where
    W: Write + Send + 'static,
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
) -> io::Result<JoinHandle<io::Result<usize>>>
where
    W: Write + Send + 'static,
    Error: FnOnce(String) + Send + 'static,
    Spawn: FnOnce(
        Box<dyn FnOnce() -> io::Result<usize> + Send>,
    ) -> io::Result<JoinHandle<io::Result<usize>>>,
{
    spawn(Box::new(move || {
        let result = process_serial_writes(&mut port, queue, &running);
        drop(port);
        if let Err(error) = &result {
            on_error(error.to_string());
        }
        result
    }))
}

pub(super) async fn stop_serial_writer<Cancel>(
    writer: JoinHandle<io::Result<usize>>,
    mut cancel: Cancel,
) -> Result<(), String>
where
    Cancel: FnMut(&JoinHandle<io::Result<usize>>) -> io::Result<()>,
{
    let mut cancellation_error = None;
    while !writer.is_finished() {
        if let Err(error) = cancel(&writer) {
            cancellation_error
                .get_or_insert_with(|| format!("Failed to cancel Serial I/O: {error}"));
        }
        tokio::time::sleep(SERIAL_IO_TIMEOUT).await;
    }
    let result = writer
        .join()
        .map_err(|_| "The Serial writer panicked".to_string())?
        .map(|_| ())
        .map_err(|error| format!("Serial write failed: {error}"));
    cancellation_error.map_or(result, Err)
}

pub(super) async fn finish_serial_workers<Cancel, Clear>(
    writer: JoinHandle<io::Result<usize>>,
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

    // The join handle owns a dedicated thread, never a reusable Tokio pool thread.
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
mod tests;
