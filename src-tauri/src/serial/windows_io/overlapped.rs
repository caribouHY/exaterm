use std::io::{self, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use serialport::SerialPort;
use windows_sys::Win32::Devices::Communication::{
    GetCommState, SetCommState, COMMTIMEOUTS, DCB, EVENPARITY, NOPARITY, ODDPARITY, ONESTOPBIT,
    TWOSTOPBITS,
};
use windows_sys::Win32::Foundation::{
    ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, OPEN_EXISTING,
};
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use super::SerialTimeoutPort;

pub(in crate::serial) struct OverlappedPort {
    pub(super) control: serialport::COMPort,
    event: OwnedHandle,
    deadline: Instant,
    pub(super) cancel: Arc<AtomicBool>,
}

impl OverlappedPort {
    pub(in crate::serial) fn open(
        name: &str,
        config: &crate::serial::SerialConfig,
    ) -> io::Result<Self> {
        let name = if name.starts_with('\\') {
            name.to_owned()
        } else {
            format!(r"\\.\{name}")
        };
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        if wide[..wide.len() - 1].contains(&0) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // SAFETY: The terminated path lives throughout the call; the returned handle is owned below.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Ownership transfers exactly once. Only configuration/control methods of COMPort
        // are used: its synchronous Read/Write implementations cannot operate this handle.
        let mut control = unsafe { serialport::COMPort::from_raw_handle(handle) };
        let mut dcb = DCB {
            DCBlength: std::mem::size_of::<DCB>() as u32,
            ..DCB::default()
        };
        // SAFETY: The port owns the handle and dcb is valid writable storage.
        if unsafe { GetCommState(handle, &mut dcb) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // Match serialport's initialization, including preserving NUL bytes and disabling DSR flow.
        dcb._bitfield = (dcb._bitfield | 1)
            & !((1 << 3) | (3 << 4) | (1 << 6) | (1 << 10) | (1 << 11) | (1 << 14));
        dcb.XonChar = 17;
        dcb.XoffChar = 19;
        dcb.ErrorChar = 0;
        dcb.EofChar = 26;
        dcb.BaudRate = config.baud_rate;
        dcb.ByteSize = match crate::serial::to_data_bits(config.data_bits) {
            serialport::DataBits::Five => 5,
            serialport::DataBits::Six => 6,
            serialport::DataBits::Seven => 7,
            serialport::DataBits::Eight => 8,
        };
        dcb.Parity = match crate::serial::to_parity(&config.parity) {
            serialport::Parity::None => NOPARITY,
            serialport::Parity::Odd => ODDPARITY,
            serialport::Parity::Even => EVENPARITY,
        };
        dcb.StopBits = match crate::serial::to_stop_bits(config.stop_bits) {
            serialport::StopBits::Two => TWOSTOPBITS,
            serialport::StopBits::One => ONESTOPBIT,
        };
        dcb._bitfield &= !((1 << 1) | (1 << 2) | (1 << 8) | (1 << 9) | (3 << 12));
        dcb._bitfield |= u32::from(dcb.Parity != NOPARITY) << 1;
        match crate::serial::to_flow_control(&config.flow_control) {
            serialport::FlowControl::Hardware => dcb._bitfield |= (1 << 2) | (1 << 12),
            serialport::FlowControl::Software => dcb._bitfield |= (1 << 8) | (1 << 9),
            serialport::FlowControl::None => {}
        }
        // Apply the final settings together: intermediate data-bit/stop-bit combinations may be invalid.
        // SAFETY: SetCommState copies the initialized DCB while the handle remains owned.
        if unsafe { SetCommState(handle, &dcb) } == 0 {
            return Err(io::Error::last_os_error());
        }
        control.set_timeout(crate::serial::SERIAL_READ_TIMEOUT)?;
        Self::new(control)
    }

    fn new(control: serialport::COMPort) -> io::Result<Self> {
        // SAFETY: An unnamed manual-reset event needs no borrowed name or security attributes.
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: CreateEventW returned a new owned handle.
        let event = unsafe { OwnedHandle::from_raw_handle(event) };
        Ok(Self {
            control,
            event,
            deadline: Instant::now() + Duration::from_secs(5),
            cancel: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(in crate::serial) fn clone_port(&self) -> io::Result<Self> {
        Self::new(self.control.try_clone_native()?)
    }

    pub(in crate::serial) fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        self.control.try_clone()
    }

    fn operation(
        &mut self,
        deadline: Option<Instant>,
        start: impl FnOnce(HANDLE, *mut OVERLAPPED) -> i32,
    ) -> io::Result<usize> {
        if deadline
            .is_some_and(|cutoff| Instant::now() >= cutoff || self.cancel.load(Ordering::SeqCst))
        {
            return Err(io::Error::from_raw_os_error(ERROR_OPERATION_ABORTED as i32));
        }
        let handle = self.control.as_raw_handle();
        let mut overlapped = OVERLAPPED {
            hEvent: self.event.as_raw_handle(),
            ..OVERLAPPED::default()
        };
        let started = start(handle, &mut overlapped);
        if started == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
                return Err(error);
            }
        }
        // SAFETY: Each direction has a separate event and only one outstanding operation. The
        // buffer captured by start and the stack OVERLAPPED remain alive until completion below.
        let mut wait = WAIT_OBJECT_0;
        if started == 0 {
            loop {
                let timeout = match deadline {
                    Some(cutoff) => {
                        if self.cancel.load(Ordering::SeqCst) || Instant::now() >= cutoff {
                            wait = WAIT_TIMEOUT;
                            break;
                        }
                        cutoff
                            .saturating_duration_since(Instant::now())
                            .as_nanos()
                            .div_ceil(1_000_000)
                            .clamp(1, 5) as u32
                    }
                    None => INFINITE,
                };
                // SAFETY: The event stays owned for the whole transfer and is unique to this direction.
                wait = unsafe { WaitForSingleObject(overlapped.hEvent, timeout) };
                if wait != WAIT_TIMEOUT {
                    break;
                }
            }
        }
        let wait_error = if wait != WAIT_OBJECT_0 && wait != WAIT_TIMEOUT {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        if wait != WAIT_OBJECT_0 {
            // SAFETY: Cancel only this operation, leaving the other direction intact. Cancellation
            // is followed by a blocking completion drain before any buffer/event can be reused.
            unsafe {
                CancelIoEx(handle, &overlapped);
            }
        }
        let mut transferred = 0;
        // SAFETY: Even after cancellation this must complete before the stack and buffer go away.
        let completed = unsafe { GetOverlappedResult(handle, &overlapped, &mut transferred, 1) };
        let completion_error = if completed == 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        if let Some(error) = wait_error {
            return Err(error);
        }
        if let Some(error) = completion_error {
            return Err(error);
        }
        Ok(transferred as usize)
    }
}

impl Read for OverlappedPort {
    fn read(&mut self, data: &mut [u8]) -> io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        let size = data.len().min(u32::MAX as usize) as u32;
        let result = self.operation(None, |handle, overlapped| {
            // SAFETY: operation retains the buffer borrow and drains completion before returning.
            unsafe { ReadFile(handle, data.as_mut_ptr(), size, ptr::null_mut(), overlapped) }
        })?;
        if result == 0 {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(result)
        }
    }
}

impl Write for OverlappedPort {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let size = data.len().min(u32::MAX as usize) as u32;
        self.operation(Some(self.deadline), |handle, overlapped| {
            // SAFETY: operation retains the buffer borrow and drains completion before returning.
            unsafe { WriteFile(handle, data.as_ptr(), size, ptr::null_mut(), overlapped) }
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.control.flush()
    }
}

impl SerialTimeoutPort for OverlappedPort {
    fn set_deadline(&mut self, deadline: Instant) {
        self.deadline = deadline;
    }
    fn get_timeouts(&self) -> io::Result<COMMTIMEOUTS> {
        self.control.get_timeouts()
    }
    fn set_timeouts(&mut self, timeouts: &COMMTIMEOUTS) -> io::Result<()> {
        self.control.set_timeouts(timeouts)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError};

    async fn fixture() -> (NamedPipeServer, OverlappedPort) {
        let name = format!(r"\\.\pipe\exaterm-overlapped-{}", uuid::Uuid::new_v4());
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .in_buffer_size(512)
            .create(&name)
            .unwrap();
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        // SAFETY: The synthetic pipe name is terminated and the new handle transfers to COMPort.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        };
        assert_ne!(handle, INVALID_HANDLE_VALUE);
        // SAFETY: The handle is newly owned, and only handle control and overlapped I/O are used.
        let port =
            OverlappedPort::new(unsafe { serialport::COMPort::from_raw_handle(handle) }).unwrap();
        server.connect().await.unwrap();
        (server, port)
    }

    fn pending_read(mut reader: OverlappedPort) -> std::thread::JoinHandle<io::Result<usize>> {
        let (ready, entered) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut buffer = [0u8; 1];
            let result = reader.operation(None, |handle, overlapped| {
                // SAFETY: The buffer and OVERLAPPED remain borrowed until operation drains them.
                let started = unsafe {
                    ReadFile(handle, buffer.as_mut_ptr(), 1, ptr::null_mut(), overlapped)
                };
                // Preserve the native result across the test synchronization call.
                let error = unsafe { GetLastError() };
                assert_eq!(started, 0);
                assert_eq!(error, ERROR_IO_PENDING);
                // A timed-out receiver must not unwind while native I/O still borrows the buffer.
                let _ = ready.send(());
                unsafe {
                    SetLastError(error);
                }
                started
            });
            assert_eq!(buffer, [b'r']);
            result
        });
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        worker
    }

    #[tokio::test]
    async fn write_completes_while_the_same_file_object_has_a_pending_read() {
        let (mut server, mut port) = fixture().await;
        let read = pending_read(port.clone_port().unwrap());
        let write = tokio::task::spawn_blocking(move || port.write(b"ping"));
        let mut received = [0; 4];
        tokio::time::timeout(Duration::from_secs(5), server.read_exact(&mut received))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received, *b"ping");
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), write)
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            4
        );
        server.write_all(b"r").await.unwrap();
        assert_eq!(read.join().unwrap().unwrap(), 1);
    }

    #[tokio::test]
    async fn expired_write_drains_cancellation_without_cancelling_the_pending_read() {
        let (mut server, mut port) = fixture().await;
        let read = pending_read(port.clone_port().unwrap());
        port.deadline = Instant::now() + Duration::from_millis(20);
        // The peer deliberately does not drain its small receive buffer.
        let write = tokio::task::spawn_blocking(move || port.write(&vec![42; 1024 * 1024]));
        let result = tokio::time::timeout(Duration::from_secs(5), write)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.unwrap_err().raw_os_error(), Some(995));
        server.write_all(b"r").await.unwrap();
        assert_eq!(read.join().unwrap().unwrap(), 1);
    }

    #[tokio::test]
    async fn cutoff_cancellation_is_a_clean_writer_shutdown() {
        use crate::serial::writer::{spawn_serial_writer, stop_serial_writer, SerialWritePort};

        struct PendingWrite {
            port: OverlappedPort,
            entered: Option<mpsc::Sender<()>>,
        }
        impl Write for PendingWrite {
            fn write(&mut self, data: &[u8]) -> io::Result<usize> {
                let entered = &mut self.entered;
                self.port
                    .operation(Some(self.port.deadline), |handle, overlapped| {
                        // SAFETY: The caller keeps data alive until operation drains completion.
                        let started = unsafe {
                            WriteFile(
                                handle,
                                data.as_ptr(),
                                data.len() as u32,
                                ptr::null_mut(),
                                overlapped,
                            )
                        };
                        let error = unsafe { GetLastError() };
                        if started == 0 {
                            assert_eq!(error, ERROR_IO_PENDING);
                            if let Some(entered) = entered.take() {
                                let _ = entered.send(());
                            }
                        }
                        unsafe {
                            SetLastError(error);
                        }
                        started
                    })
            }
            fn flush(&mut self) -> io::Result<()> {
                panic!("shutdown must not flush queued data")
            }
        }
        impl SerialWritePort for PendingWrite {}

        let (_server, mut port) = fixture().await;
        port.deadline = Instant::now() + Duration::from_secs(30);
        let cancel = port.cancel.clone();
        let running = Arc::new(AtomicBool::new(true));
        let (entered, pending) = mpsc::channel();
        let (send, queue) = mpsc::channel();
        send.send(vec![42; 1024 * 1024]).unwrap();
        drop(send);
        let worker = spawn_serial_writer(
            PendingWrite {
                port,
                entered: Some(entered),
            },
            queue,
            running.clone(),
            |_| panic!("expected cancellation must not notify a write error"),
        )
        .unwrap();
        pending.recv_timeout(Duration::from_secs(5)).unwrap();
        running.store(false, Ordering::SeqCst);
        tokio::time::timeout(
            Duration::from_secs(5),
            stop_serial_writer(worker, move |_| {
                cancel.store(true, Ordering::SeqCst);
                Ok(())
            }),
        )
        .await
        .unwrap()
        .unwrap();
    }

    #[tokio::test]
    async fn expired_absolute_deadline_prevents_a_new_submission() {
        let (_server, mut port) = fixture().await;
        port.set_deadline(Instant::now() - Duration::from_millis(1));
        assert_eq!(
            port.write(b"x").unwrap_err().raw_os_error(),
            Some(ERROR_OPERATION_ABORTED as i32)
        );
    }
}
