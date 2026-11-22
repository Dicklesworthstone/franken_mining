//! Determinism: BitwisePortable
//! Headframe S1 site telemetry segment log, bounded ingest backpressure with continuity receipts,
//! raw ring-buffer retention, and 1-minute rollups (`BD-242`, plan §26.7–§26.8, `PO-BED-7`).

pub mod backpressure;
pub mod endurance;
pub mod rollup;
pub mod segment;

use core::fmt;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::UnitsError;

/// Typed errors across `fmine-telemetry` segment sealing, backpressure, and rollups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryError {
    /// Invalid policy, capacity, stride, or sample encoding parameter.
    InvalidConfig { field: &'static str, detail: String },
    /// Non-monotonic timestamp within a signal stream or invalid site identifier.
    OutOfOrderTimestamp {
        signal_id: u32,
        prev_ts_ns: u64,
        got_ts_ns: u64,
    },
    /// Conflicting fixed-point `scale_exp10` for the same `signal_id`.
    ScaleMismatch {
        signal_id: u32,
        expected_exp10: i8,
        got_exp10: i8,
    },
    /// Trace reference does not match the segment root, signal, or offset bounds.
    InvalidTraceRef { detail: String },
    /// Segment root or continuity receipt digest verification failed.
    DigestMismatch { kind: &'static str, detail: String },
    /// Exact integer or rational arithmetic overflow.
    Overflow,
}

impl TelemetryError {
    /// Maps every telemetry error to its registered `FMINE-REF-*` refusal.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::InvalidConfig { .. } | Self::Overflow => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::OutOfOrderTimestamp { .. } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::ScaleMismatch { .. } => {
                Refusal::new(RefusalCode::BasisMismatch105, self.to_string())
            }
            Self::InvalidTraceRef { .. } => {
                Refusal::new(RefusalCode::NoClaimInDomain201, self.to_string())
            }
            Self::DigestMismatch { .. } => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
        }
    }
}

impl From<UnitsError> for TelemetryError {
    fn from(value: UnitsError) -> Self {
        match value {
            UnitsError::Overflow => Self::Overflow,
            other => Self::InvalidConfig {
                field: "units",
                detail: other.to_string(),
            },
        }
    }
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field, detail } => {
                write!(f, "invalid telemetry config/input '{field}': {detail}")
            }
            Self::OutOfOrderTimestamp {
                signal_id,
                prev_ts_ns,
                got_ts_ns,
            } => write!(
                f,
                "out-of-order timestamp for signal {signal_id}: {got_ts_ns} < {prev_ts_ns}"
            ),
            Self::ScaleMismatch {
                signal_id,
                expected_exp10,
                got_exp10,
            } => write!(
                f,
                "scale mismatch for signal {signal_id}: expected 10^{expected_exp10}, got 10^{got_exp10}"
            ),
            Self::InvalidTraceRef { detail } => write!(f, "invalid trace reference: {detail}"),
            Self::DigestMismatch { kind, detail } => {
                write!(f, "{kind} digest mismatch: {detail}")
            }
            Self::Overflow => write!(f, "telemetry arithmetic overflow"),
        }
    }
}

impl std::error::Error for TelemetryError {}
