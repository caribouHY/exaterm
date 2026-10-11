use std::io::{self, Write};
use std::os::windows::io::AsRawHandle;
use std::time::Duration;

use serialport::SerialPort;
use windows_sys::Win32::Devices::Communication::{GetCommTimeouts, SetCommTimeouts, COMMTIMEOUTS};

use super::writer::{SerialWritePort, WritePolicy};

pub(super) trait SerialTimeoutPort: Write {
    fn get_timeouts(&self) -> io::Result<COMMTIMEOUTS>;
    fn set_timeouts(&mut self, timeouts: &COMMTIMEOUTS) -> io::Result<()>;
}

impl SerialTimeoutPort for serialport::COMPort {
    fn get_timeouts(&self) -> io::Result<COMMTIMEOUTS> {
        let mut timeouts = COMMTIMEOUTS::default();
        // SAFETY: The borrowed COMPort owns this handle and the output is a valid structure.
        if unsafe { GetCommTimeouts(self.as_raw_handle(), &mut timeouts) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(timeouts)
        }
    }

    fn set_timeouts(&mut self, timeouts: &COMMTIMEOUTS) -> io::Result<()> {
        // SAFETY: The COMPort retains its handle throughout the call; Windows copies the structure.
        if unsafe { SetCommTimeouts(self.as_raw_handle(), timeouts) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

pub(super) struct WindowsWriterPort<P = serialport::COMPort> {
    port: P,
    policy: WritePolicy,
}

impl WindowsWriterPort {
    pub(super) fn new(port: serialport::COMPort) -> serialport::Result<Self> {
        let policy = WritePolicy::new(
            port.baud_rate()?,
            port.data_bits()?,
            port.parity()?,
            port.stop_bits()?,
        )?;
        Ok(Self { port, policy })
    }
}

impl<P: Write> Write for WindowsWriterPort<P> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.port.write(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.port.flush()
    }
}

impl<P: SerialTimeoutPort> SerialWritePort for WindowsWriterPort<P> {
    fn policy(&self) -> Option<WritePolicy> {
        Some(self.policy)
    }

    fn set_write_timeout(&mut self, remaining: Duration) -> io::Result<()> {
        let mut timeouts = self.port.get_timeouts()?;
        // Cloned handles share COMMTIMEOUTS, so preserve the reader's polling settings.
        timeouts.WriteTotalTimeoutMultiplier = 0;
        timeouts.WriteTotalTimeoutConstant = remaining
            .as_nanos()
            .div_ceil(1_000_000)
            .clamp(1, u128::from(u32::MAX - 1)) as u32;
        self.port.set_timeouts(&timeouts)
    }
}

#[cfg(test)]
mod tests;
