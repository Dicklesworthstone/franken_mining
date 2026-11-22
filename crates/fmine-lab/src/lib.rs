//! Determinism: BitwisePortable
//! Testpit `S1` deterministic virtual-time `LabRuntime` harness, DPOR schedule explorer,
//! chaos profiles + `CrashPack` reproducibility, runtime capability-mask lab test (`PO-BED-8`),
//! and `INT-AS-001` criterion 8 scheduler/diagnostic invariance (`BD-099`, `BD-100`, `BD-097`, `BD-337`).

pub mod cap_audit;
pub mod chaos;
pub mod invariance;
pub mod oracles;
pub mod runtime;

use core::fmt;

use fmine_evidence::{Refusal, RefusalCode};

/// Typed errors raised by `fmine-lab` harnesses, virtual clocks, `CrashPack` codecs, and audits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabError {
    /// Virtual clock attempted to step backward.
    TimeWentBack { current_ns: i64, requested_ns: i64 },
    /// Invalid harness or chaos configuration.
    InvalidConfig { field: &'static str, detail: String },
    /// Simulation exceeded its hard deterministic step budget.
    StepBudgetExceeded { max_steps: usize },
    /// Corrupted or tampered serialized `CrashPack`.
    CorruptCrashPack { detail: String },
    /// Runtime capability-mask (`PO-BED-8`) invariant failure.
    CapabilityViolation { detail: String },
    /// Scheduler or diagnostic adaptation perturbed an authoritative output (`INT-AS-001` crit 8).
    InvarianceViolation { detail: String },
}

impl LabError {
    /// Map lab errors to registered `FMINE-REF-*` codes.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::TimeWentBack { .. } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::CorruptCrashPack { .. } => {
                Refusal::new(RefusalCode::DigestMismatch505, self.to_string())
            }
            Self::CapabilityViolation { .. } => {
                Refusal::new(RefusalCode::MnpiWall301, self.to_string())
            }
            Self::InvarianceViolation { .. } => {
                Refusal::new(RefusalCode::NumericalCertificateFailed207, self.to_string())
            }
            Self::InvalidConfig { .. } | Self::StepBudgetExceeded { .. } => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
        }
    }
}

impl fmt::Display for LabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimeWentBack {
                current_ns,
                requested_ns,
            } => write!(
                f,
                "virtual clock cannot move backward from {current_ns} ns to {requested_ns} ns"
            ),
            Self::InvalidConfig { field, detail } => {
                write!(f, "invalid lab config '{field}': {detail}")
            }
            Self::StepBudgetExceeded { max_steps } => {
                write!(f, "lab simulation exceeded max_steps={max_steps}")
            }
            Self::CorruptCrashPack { detail } => write!(f, "corrupt CrashPack: {detail}"),
            Self::CapabilityViolation { detail } => {
                write!(f, "PO-BED-8 capability violation: {detail}")
            }
            Self::InvarianceViolation { detail } => {
                write!(f, "INT-AS-001 criterion 8 invariance violation: {detail}")
            }
        }
    }
}

impl std::error::Error for LabError {}
