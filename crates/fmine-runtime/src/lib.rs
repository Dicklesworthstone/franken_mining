//! DESIGN: the runtime owns the explicit host wall-clock adapter. A reading provides no claim of clock
//! synchronization, bounded uncertainty, monotonicity, authority or external time proof.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A failed conversion to nonnegative signed Unix nanoseconds; no timestamp is clamped or invented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockError {
    BeforeUnixEpoch,
    NanosecondOverflow,
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BeforeUnixEpoch => "host wall clock precedes the Unix epoch",
            Self::NanosecondOverflow => "host wall clock exceeds signed 64-bit Unix nanoseconds",
        })
    }
}

impl std::error::Error for ClockError {}

/// Explicitly injected clock capability. Callers separately decide whether and how to trust its reading.
pub trait WallClock {
    fn now_ns(&self) -> Result<i64, ClockError>;
}

/// Reads the actual host wall clock. This adapter does not upgrade its quality to a trusted time bound.
#[derive(Debug, Clone, Copy, Default)]
pub struct HostWallClock;

impl WallClock for HostWallClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        unix_ns(SystemTime::now())
    }
}

fn unix_ns(time: SystemTime) -> Result<i64, ClockError> {
    duration_ns(
        time.duration_since(UNIX_EPOCH)
            .map_err(|_| ClockError::BeforeUnixEpoch)?,
    )
}

fn duration_ns(duration: Duration) -> Result<i64, ClockError> {
    i64::try_from(duration.as_nanos()).map_err(|_| ClockError::NanosecondOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_zero_and_pre_epoch_is_refused() {
        assert_eq!(unix_ns(UNIX_EPOCH), Ok(0));
        let before = UNIX_EPOCH
            .checked_sub(Duration::from_secs(1))
            .expect("one second before epoch");
        assert_eq!(unix_ns(before), Err(ClockError::BeforeUnixEpoch));
    }

    #[test]
    fn nanosecond_conversion_is_exact_and_never_wraps_or_clamps() {
        assert_eq!(
            duration_ns(Duration::new(1, 234_567_890)),
            Ok(1_234_567_890)
        );
        assert_eq!(
            duration_ns(Duration::from_nanos(i64::MAX as u64)),
            Ok(i64::MAX)
        );
        assert_eq!(
            duration_ns(Duration::from_nanos(i64::MAX as u64 + 1)),
            Err(ClockError::NanosecondOverflow)
        );
        assert_eq!(
            duration_ns(Duration::MAX),
            Err(ClockError::NanosecondOverflow)
        );
    }

    #[test]
    fn injected_clock_keeps_its_reading_or_failure_without_a_host_clock_dependency() {
        struct FixedClock(Result<i64, ClockError>);
        impl WallClock for FixedClock {
            fn now_ns(&self) -> Result<i64, ClockError> {
                self.0
            }
        }
        let value: &dyn WallClock = &FixedClock(Ok(42));
        let failed: &dyn WallClock = &FixedClock(Err(ClockError::BeforeUnixEpoch));
        assert_eq!(value.now_ns(), Ok(42));
        assert_eq!(failed.now_ns(), Err(ClockError::BeforeUnixEpoch));
    }
}
