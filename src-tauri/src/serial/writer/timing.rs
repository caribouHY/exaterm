use std::num::NonZeroU32;
use std::time::Duration;

use super::WRITE_CHUNK_SIZE;

const MIN_WRITE_TIMEOUT_MS: u128 = 5000;
const MAX_WRITE_TIMEOUT_MS: u128 = 30000;
const WRITE_MARGIN_MS: u128 = 1000;

#[derive(Clone, Copy)]
pub(in crate::serial) struct WritePolicy {
    baud: NonZeroU32,
    frame_bits: u128,
}

impl WritePolicy {
    #[cfg(any(windows, test))]
    pub(in crate::serial) fn new(
        baud: u32,
        data_bits: serialport::DataBits,
        parity: serialport::Parity,
        stop_bits: serialport::StopBits,
    ) -> std::io::Result<Self> {
        let baud = NonZeroU32::new(baud).ok_or(std::io::ErrorKind::InvalidInput)?;
        let data_bits = match data_bits {
            serialport::DataBits::Five => 5,
            serialport::DataBits::Six => 6,
            serialport::DataBits::Seven => 7,
            serialport::DataBits::Eight => 8,
        };
        let parity_bits = u128::from(parity != serialport::Parity::None);
        let stop_bits = match stop_bits {
            serialport::StopBits::One => 1,
            serialport::StopBits::Two => 2,
        };
        Ok(Self {
            baud,
            frame_bits: 1 + data_bits + parity_bits + stop_bits,
        })
    }

    pub(super) fn chunk_size(self) -> usize {
        let size = (MAX_WRITE_TIMEOUT_MS - WRITE_MARGIN_MS) * u128::from(self.baud.get())
            / (2 * self.frame_bits * 1000);
        size.clamp(1, WRITE_CHUNK_SIZE as u128) as usize
    }

    pub(super) fn timeout(self, size: usize) -> Duration {
        let transmission_ms =
            (2 * size as u128 * self.frame_bits * 1000).div_ceil(u128::from(self.baud.get()));
        Duration::from_millis(
            (WRITE_MARGIN_MS + transmission_ms).clamp(MIN_WRITE_TIMEOUT_MS, MAX_WRITE_TIMEOUT_MS)
                as u64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{DataBits, Parity, StopBits};

    #[test]
    fn transmission_budget_and_slow_baud_chunks() {
        for (baud, chunk, timeout) in [(115200, 4096, 5000), (9600, 4096, 9534), (300, 435, 30000)]
        {
            let policy =
                WritePolicy::new(baud, DataBits::Eight, Parity::None, StopBits::One).unwrap();
            assert_eq!(policy.chunk_size(), chunk);
            assert_eq!(policy.timeout(chunk), Duration::from_millis(timeout));
            assert_eq!(policy.timeout(1), Duration::from_secs(5));
        }
    }

    #[test]
    fn framing_and_extreme_baud_rates_keep_budgets_finite() {
        for baud in [1, 50, 300, 9600, u32::MAX] {
            for data in [
                DataBits::Five,
                DataBits::Six,
                DataBits::Seven,
                DataBits::Eight,
            ] {
                for parity in [Parity::None, Parity::Odd, Parity::Even] {
                    for stop in [StopBits::One, StopBits::Two] {
                        let policy = WritePolicy::new(baud, data, parity, stop).unwrap();
                        let chunk = policy.chunk_size();
                        assert!((1..=4096).contains(&chunk));
                        assert!((Duration::from_secs(5)..=Duration::from_secs(30))
                            .contains(&policy.timeout(chunk)));
                    }
                }
            }
        }
        let policy = WritePolicy::new(9600, DataBits::Eight, Parity::Even, StopBits::Two).unwrap();
        assert_eq!(policy.timeout(4096), Duration::from_millis(11240));
        assert!(WritePolicy::new(0, DataBits::Eight, Parity::None, StopBits::One).is_err());
    }
}
